use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use rumahl_app_operations::{AppOperationRunner, AppRuntimeServices, RuntimeSecretDelivery};
use rumahl_core::{
    AppId, AppManifest, AppRuntimeInstallationState, AppVersion, InMemoryGrantStore,
    InstallationId, InstalledApp, PackagePath, PlatformState, PublisherId, RuntimeDescriptor,
    RuntimeEntrypoint, RuntimeEntrypointId,
};
use rumahl_oidc_provider::{InstalledAppOriginResolver, OidcClientId, OidcClientSecret};
use rumahl_persistence_sqlite::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqliteAppDatabaseProvider, SqliteAppOperationRepository, SqliteOidcClientRepository,
    SqliteSecretStore, SqliteSnapshotRepository,
};
use rumahl_platform_buildroot::{
    DockerImageReference, DockerImageResolver, DockerRuntimeTarget, DockerRuntimeTargetConfig,
    RuntimeControlTarget, RuntimeControlTargetOutcome, RuntimeInstallationSpec,
    UnixAppRuntimeProvider, UnixAppRuntimeProviderConfig, UnixRuntimeControlServer,
    UnixRuntimeControlServerConfig,
};

const E2E_IMAGE_ENV: &str = "RUMAHL_CONTAINER_E2E_IMAGE";
const E2E_DOCKER_ENV: &str = "RUMAHL_CONTAINER_E2E_DOCKER";
const EXPECTED_ARTIFACT: &str = "runtime/server.oci";
const INSTANCE_LABEL: &str = "io.rumahl.supervisor";

#[derive(Debug)]
struct UnexpectedCall;

impl fmt::Display for UnexpectedCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "an undeclared OIDC operation was attempted")
    }
}

impl Error for UnexpectedCall {}

#[derive(Debug, Clone)]
struct PinnedImageResolver {
    image: DockerImageReference,
}

#[derive(Debug)]
struct ImageResolutionError;

impl fmt::Display for ImageResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the E2E manifest referenced an unexpected artifact")
    }
}

impl Error for ImageResolutionError {}

impl DockerImageResolver for PinnedImageResolver {
    type Error = ImageResolutionError;

    fn resolve_image(
        &self,
        _spec: &RuntimeInstallationSpec,
        artifact: &PackagePath,
    ) -> Result<DockerImageReference, Self::Error> {
        if artifact.as_str() != EXPECTED_ARTIFACT {
            return Err(ImageResolutionError);
        }
        Ok(self.image.clone())
    }
}

struct Cleanup {
    root: PathBuf,
    docker: PathBuf,
    supervisor_instance: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let filter = format!("label={INSTANCE_LABEL}={}", self.supervisor_instance);
        if let Ok(output) = docker_output(
            &self.docker,
            &[
                "container",
                "ls",
                "--all",
                "--filter",
                &filter,
                "--format",
                "{{.Names}}",
            ],
        ) {
            for name in output.lines().filter(|name| !name.is_empty()) {
                let _ = docker_output(&self.docker, &["container", "rm", "--force", name]);
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct TestKeyProvider;

impl SecretEncryptionKeyProvider for TestKeyProvider {
    type Error = Infallible;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
        Ok(test_key())
    }

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        Ok((id.as_str() == "container-e2e-key").then(test_key))
    }
}

fn test_key() -> SecretEncryptionKey {
    SecretEncryptionKey::new(
        SecretEncryptionKeyId::parse("container-e2e-key").unwrap(),
        [0x31; 32],
    )
}

struct NoOidcOrigin;

impl InstalledAppOriginResolver for NoOidcOrigin {
    type Error = UnexpectedCall;

    fn resolve_origin(
        &self,
        _app: &InstalledApp,
        _entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error> {
        Err(UnexpectedCall)
    }
}

struct NoRuntimeSecrets;

impl RuntimeSecretDelivery for NoRuntimeSecrets {
    type Error = UnexpectedCall;

    fn deliver_oidc_client_secret(
        &self,
        _operation_id: &rumahl_core::AppOperationId,
        _app: &InstalledApp,
        _client_id: &OidcClientId,
        _client_secret: &OidcClientSecret,
    ) -> Result<(), Self::Error> {
        Err(UnexpectedCall)
    }

    fn remove_for_installation(
        &self,
        _operation_id: &rumahl_core::AppOperationId,
        _installation_id: &InstallationId,
    ) -> Result<(), Self::Error> {
        Err(UnexpectedCall)
    }
}

fn current_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions.
    unsafe { libc::geteuid() }
}

fn test_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("re2e-{}", InstallationId::new()));
    fs::create_dir(&root).unwrap();
    root
}

fn docker_output(docker: &Path, arguments: &[&str]) -> Result<String, UnexpectedCall> {
    let output = Command::new(docker)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .map_err(|_| UnexpectedCall)?;
    if !output.status.success() {
        return Err(UnexpectedCall);
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| UnexpectedCall)
}

fn wait_until_ready(docker: &Path, installation_id: &InstallationId) -> Result<(), UnexpectedCall> {
    let name = format!("rumahl-app-{installation_id}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if docker_output(
            docker,
            &[
                "container",
                "exec",
                &name,
                "/bin/sh",
                "-c",
                "test -f /tmp/rumahl-ready",
            ],
        )
        .is_ok()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(UnexpectedCall)
}

fn container_manifest() -> AppManifest {
    let mut runtime = RuntimeDescriptor::container();
    runtime
        .add_entrypoint(RuntimeEntrypoint::container_artifact(
            RuntimeEntrypointId::parse("service").unwrap(),
            PackagePath::parse(EXPECTED_ARTIFACT).unwrap(),
        ))
        .unwrap();
    AppManifest::new(
        AppId::parse("com.rumahl.container-e2e").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Container E2E",
        runtime,
    )
    .unwrap()
}

fn assert_state(outcome: RuntimeControlTargetOutcome, expected: AppRuntimeInstallationState) {
    assert!(matches!(
        outcome,
        RuntimeControlTargetOutcome::Accepted { state, .. } if state == expected
    ));
}

#[test]
#[ignore = "requires a Docker daemon and RUMAHL_CONTAINER_E2E_IMAGE"]
fn installs_runs_and_uninstalls_real_container_app() {
    let image = DockerImageReference::parse(
        std::env::var(E2E_IMAGE_ENV)
            .unwrap_or_else(|_| panic!("{E2E_IMAGE_ENV} must name a pre-pulled immutable image")),
    )
    .unwrap();
    let docker = std::env::var_os(E2E_DOCKER_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/docker"));
    let root = test_root();
    let runtime_root = root.join("runtime");
    fs::create_dir(&runtime_root).unwrap();
    let supervisor_instance = format!("e2e-{}", InstallationId::new());
    let _cleanup = Cleanup {
        root: root.clone(),
        docker: docker.clone(),
        supervisor_instance: supervisor_instance.clone(),
    };
    let config =
        DockerRuntimeTargetConfig::new(&docker, &runtime_root, "none", supervisor_instance)
            .unwrap();
    let target = DockerRuntimeTarget::new(config, PinnedImageResolver { image });
    target.probe().unwrap();

    let socket_path = root.join("runtime.sock");
    let install_server = UnixRuntimeControlServer::bind(
        UnixRuntimeControlServerConfig::new(&socket_path, current_uid()).unwrap(),
        target.clone(),
    )
    .unwrap();
    let install_server_thread = thread::spawn(move || {
        for _ in 0..2 {
            install_server.serve_once().unwrap();
        }
    });
    let provider = UnixAppRuntimeProvider::new(
        UnixAppRuntimeProviderConfig::new(&socket_path, current_uid()).unwrap(),
    );
    let state_path = root.join("platform.sqlite3");
    let runner = AppOperationRunner::new(
        SqliteAppOperationRepository::open(&state_path).unwrap(),
        SqliteAppDatabaseProvider::open(root.join("databases")).unwrap(),
        AppRuntimeServices::new(provider, NoRuntimeSecrets),
        SqliteOidcClientRepository::open(&state_path).unwrap(),
        NoOidcOrigin,
        SqliteSnapshotRepository::open(&state_path).unwrap(),
        SqliteSecretStore::open(&state_path, TestKeyProvider).unwrap(),
    );
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();

    let installed = runner
        .install(container_manifest(), &mut state, &grants)
        .unwrap()
        .app()
        .clone();
    install_server_thread.join().unwrap();
    wait_until_ready(&docker, installed.installation_id()).unwrap();
    assert_state(
        target
            .installation_state(*installed.installation_id())
            .unwrap(),
        AppRuntimeInstallationState::Active,
    );
    assert_eq!(state.installed_apps().len(), 1);

    // The runtime supervisor is independently restartable. Docker and the
    // platform operation runner retain their state while its socket is
    // replaced by the service-manager startup sequence.
    fs::remove_file(&socket_path).unwrap();
    let uninstall_server = UnixRuntimeControlServer::bind(
        UnixRuntimeControlServerConfig::new(&socket_path, current_uid()).unwrap(),
        target.clone(),
    )
    .unwrap();
    let uninstall_server_thread = thread::spawn(move || {
        for _ in 0..2 {
            uninstall_server.serve_once().unwrap();
        }
    });

    runner
        .uninstall(installed.installation_id(), &mut state, &mut grants)
        .unwrap();
    uninstall_server_thread.join().unwrap();
    assert_state(
        target
            .installation_state(*installed.installation_id())
            .unwrap(),
        AppRuntimeInstallationState::Absent,
    );
    assert!(state.installed_apps().is_empty());
}
