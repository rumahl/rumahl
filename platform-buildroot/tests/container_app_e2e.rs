use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rumahl_app_operations::{AppOperationRunner, AppRuntimeServices};
use rumahl_core::{
    AppId, AppManifest, AppOperationRepository, AppVersion, InMemoryGrantStore, InstallationId,
    InstalledApp, OidcCallbackPath, OidcClientDeclaration, OidcClientType, OidcScope, PackagePath,
    PlatformState, PublisherId, RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint,
    RuntimeEntrypointId,
};
use rumahl_oidc_provider::InstalledAppOriginResolver;
use rumahl_persistence_sqlite::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqliteAppDatabaseProvider, SqliteAppOperationRepository, SqliteOidcClientRepository,
    SqliteSecretStore, SqliteSnapshotRepository,
};
use rumahl_platform_buildroot::{
    UnixAppRuntimeProvider, UnixAppRuntimeProviderConfig, UnixRuntimeSecretDelivery,
    UnixRuntimeSecretDeliveryConfig,
};

const E2E_IMAGE_ENV: &str = "RUMAHL_CONTAINER_E2E_IMAGE";
const E2E_DOCKER_ENV: &str = "RUMAHL_CONTAINER_E2E_DOCKER";
const E2E_PLATFORM_USER_ENV: &str = "RUMAHL_CONTAINER_E2E_PLATFORM_USER";
const EXPECTED_ARTIFACT: &str = "runtime/server.oci";
const INSTANCE_LABEL: &str = "io.rumahl.supervisor";

#[derive(Debug)]
struct UnexpectedCall;

impl fmt::Display for UnexpectedCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "an unexpected E2E operation failed")
    }
}

impl Error for UnexpectedCall {}

struct SupervisorProcess {
    child: Option<Child>,
}

impl SupervisorProcess {
    fn stop(mut self) {
        self.terminate();
    }

    fn terminate(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for SupervisorProcess {
    fn drop(&mut self) {
        self.terminate();
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

struct TestOidcOrigin;

impl InstalledAppOriginResolver for TestOidcOrigin {
    type Error = UnexpectedCall;

    fn resolve_origin(
        &self,
        _app: &InstalledApp,
        _entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error> {
        Ok("https://container-e2e.rumahl.local".to_owned())
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

fn start_supervisor(
    docker: &Path,
    runtime_root: &Path,
    image_root: &Path,
    control_socket: &Path,
    secret_socket: &Path,
    supervisor_instance: &str,
    platform_user: &str,
) -> SupervisorProcess {
    let child = Command::new(env!("CARGO_BIN_EXE_rumahl-runtime-supervisor"))
        .arg("--docker")
        .arg(docker)
        .arg("--runtime-root")
        .arg(runtime_root)
        .arg("--image-root")
        .arg(image_root)
        .arg("--network")
        .arg("none")
        .arg("--instance")
        .arg(supervisor_instance)
        .arg("--control-socket")
        .arg(control_socket)
        .arg("--secret-socket")
        .arg(secret_socket)
        .arg("--platform-user")
        .arg(platform_user)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut process = SupervisorProcess { child: Some(child) };
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if control_socket.exists() && secret_socket.exists() {
            return process;
        }
        if let Some(status) = process.child.as_mut().unwrap().try_wait().unwrap() {
            panic!("runtime supervisor exited during startup with {status}");
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("runtime supervisor did not create its control socket")
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

fn stage_image(image_root: &Path, installation_id: &InstallationId, image: &str) {
    let installation_root = image_root.join(installation_id.to_string());
    fs::create_dir(&installation_root).unwrap();
    fs::write(
        installation_root.join("image-reference"),
        format!("RDI1\n{EXPECTED_ARTIFACT}\n{image}\n"),
    )
    .unwrap();
}

fn container_manifest() -> AppManifest {
    let mut runtime = RuntimeDescriptor::container();
    runtime
        .add_entrypoint(RuntimeEntrypoint::container_artifact(
            RuntimeEntrypointId::parse("service").unwrap(),
            PackagePath::parse(EXPECTED_ARTIFACT).unwrap(),
        ))
        .unwrap();
    runtime
        .add_entrypoint(RuntimeEntrypoint::endpoint(
            RuntimeEntrypointId::parse("main").unwrap(),
            RuntimeEndpointId::parse("web").unwrap(),
        ))
        .unwrap();
    let mut manifest = AppManifest::new(
        AppId::parse("com.rumahl.container-e2e").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Container E2E",
        runtime,
    )
    .unwrap();
    manifest
        .declare_oidc_client(
            OidcClientDeclaration::new(
                OidcClientType::Confidential,
                RuntimeEntrypointId::parse("main").unwrap(),
                OidcCallbackPath::parse("/oidc/callback").unwrap(),
                vec![OidcScope::OpenId, OidcScope::Profile],
            )
            .unwrap(),
        )
        .unwrap();
    manifest
}

#[test]
#[ignore = "requires a Docker daemon and RUMAHL_CONTAINER_E2E_IMAGE"]
fn installs_runs_and_uninstalls_real_container_app() {
    let image = std::env::var(E2E_IMAGE_ENV)
        .unwrap_or_else(|_| panic!("{E2E_IMAGE_ENV} must name a pre-pulled immutable image"));
    let docker = std::env::var_os(E2E_DOCKER_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/docker"));
    let platform_user = std::env::var(E2E_PLATFORM_USER_ENV)
        .unwrap_or_else(|_| panic!("{E2E_PLATFORM_USER_ENV} must name the current test user"));
    let root = test_root();
    let runtime_root = root.join("runtime");
    let image_root = root.join("images");
    fs::create_dir(&runtime_root).unwrap();
    fs::create_dir(&image_root).unwrap();
    let control_socket = root.join("runtime.sock");
    let secret_socket = root.join("secrets.sock");
    let supervisor_instance = format!("e2e-{}", InstallationId::new());
    let _cleanup = Cleanup {
        root: root.clone(),
        docker: docker.clone(),
        supervisor_instance: supervisor_instance.clone(),
    };
    let supervisor = start_supervisor(
        &docker,
        &runtime_root,
        &image_root,
        &control_socket,
        &secret_socket,
        &supervisor_instance,
        &platform_user,
    );
    let provider = UnixAppRuntimeProvider::new(
        UnixAppRuntimeProviderConfig::new(&control_socket, current_uid()).unwrap(),
    );
    let secret_delivery = UnixRuntimeSecretDelivery::new(
        UnixRuntimeSecretDeliveryConfig::new(&secret_socket, current_uid()).unwrap(),
    );
    let state_path = root.join("platform.sqlite3");
    let runner = AppOperationRunner::new(
        SqliteAppOperationRepository::open(&state_path).unwrap(),
        SqliteAppDatabaseProvider::open(root.join("databases")).unwrap(),
        AppRuntimeServices::new(provider, secret_delivery),
        SqliteOidcClientRepository::open(&state_path).unwrap(),
        TestOidcOrigin,
        SqliteSnapshotRepository::open(&state_path).unwrap(),
        SqliteSecretStore::open(&state_path, TestKeyProvider).unwrap(),
    );
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();

    // Image publication is intentionally a separate participant. The first
    // attempt fails closed and leaves an operation that can be resumed after
    // the package importer publishes its immutable image reference.
    runner
        .install(container_manifest(), &mut state, &grants)
        .unwrap_err();
    assert!(state.installed_apps().is_empty());
    let incomplete = runner.journal().list_incomplete().unwrap();
    assert_eq!(incomplete.len(), 1);
    let installation_id = *incomplete[0].target_app().unwrap().installation_id();
    stage_image(&image_root, &installation_id, &image);

    let report = runner.recover_incomplete(&mut state, &mut grants).unwrap();
    assert_eq!(report.resumed(), 1);
    assert_eq!(report.committed(), 1);
    let installed = state.installed_apps().apps()[0].clone();
    wait_until_ready(&docker, installed.installation_id()).unwrap();
    let container_name = format!("rumahl-app-{installation_id}");
    docker_output(
        &docker,
        &[
            "container",
            "exec",
            &container_name,
            "/bin/sh",
            "-c",
            "test -s /run/rumahl/secrets/oidc-client-id && test -s /run/rumahl/secrets/oidc-client-secret",
        ],
    )
    .unwrap();
    assert_eq!(
        docker_output(
            &docker,
            &[
                "container",
                "inspect",
                "--format",
                "{{.State.Status}}",
                &format!("rumahl-app-{installation_id}"),
            ],
        )
        .unwrap(),
        "running"
    );

    // Simulate the service manager replacing only the runtime supervisor. The
    // platform runner, SQLite journal, and Docker container remain alive.
    supervisor.stop();
    fs::remove_file(&control_socket).unwrap();
    fs::remove_file(&secret_socket).unwrap();
    let _restarted_supervisor = start_supervisor(
        &docker,
        &runtime_root,
        &image_root,
        &control_socket,
        &secret_socket,
        &supervisor_instance,
        &platform_user,
    );

    runner
        .uninstall(installed.installation_id(), &mut state, &mut grants)
        .unwrap();
    assert!(state.installed_apps().is_empty());
    assert!(!runtime_root.join(installation_id.to_string()).exists());
    assert!(
        docker_output(
            &docker,
            &[
                "container",
                "ls",
                "--all",
                "--filter",
                &format!("name=^/rumahl-app-{installation_id}$"),
                "--format",
                "{{.Names}}",
            ],
        )
        .unwrap()
        .is_empty()
    );
}
