use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rumahl_app_operations::{AppOperationRunner, AppRuntimeServices, RuntimeSecretDelivery};
use rumahl_core::{
    AppId, AppManifest, AppRuntimeInstallationState, AppVersion, InMemoryGrantStore,
    InstallationId, InstalledApp, PackagePath, PlatformState, PublisherId, RuntimeDescriptor,
    RuntimeEntrypoint, RuntimeEntrypointId, RuntimeEntrypointTarget, RuntimeKind,
};
use rumahl_oidc_provider::{InstalledAppOriginResolver, OidcClientId, OidcClientSecret};
use rumahl_persistence_sqlite::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqliteAppDatabaseProvider, SqliteAppOperationRepository, SqliteOidcClientRepository,
    SqliteSecretStore, SqliteSnapshotRepository,
};
use rumahl_platform_buildroot::{
    RuntimeControlTarget, RuntimeControlTargetOutcome, RuntimeInstallationSpec,
    UnixAppRuntimeProvider, UnixAppRuntimeProviderConfig, UnixRuntimeControlServer,
    UnixRuntimeControlServerConfig,
};

const E2E_IMAGE_ENV: &str = "RUMAHL_CONTAINER_E2E_IMAGE";
const E2E_DOCKER_ENV: &str = "RUMAHL_CONTAINER_E2E_DOCKER";
const SPEC_LABEL: &str = "io.rumahl.e2e.spec";
const INSTALLATION_LABEL: &str = "io.rumahl.installation";
const EXPECTED_ARTIFACT: &str = "runtime/server.oci";

#[derive(Debug)]
enum E2eError {
    Io(io::Error),
    DockerCommand(&'static str),
    InvalidDockerOutput(&'static str),
    InvalidRuntimeSpec,
    OriginResolutionWasUnexpected,
}

impl fmt::Display for E2eError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => write!(f, "container E2E I/O failed"),
            Self::DockerCommand(operation) => {
                write!(f, "Docker operation '{operation}' failed")
            }
            Self::InvalidDockerOutput(operation) => {
                write!(f, "Docker operation '{operation}' returned invalid output")
            }
            Self::InvalidRuntimeSpec => write!(f, "runtime specification is not the E2E app"),
            Self::OriginResolutionWasUnexpected => {
                write!(f, "OIDC origin resolution was unexpectedly requested")
            }
        }
    }
}

impl Error for E2eError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for E2eError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone)]
struct DockerE2eTarget {
    executable: PathBuf,
    image: String,
    managed_containers: Arc<Mutex<Vec<String>>>,
}

impl DockerE2eTarget {
    fn new(executable: PathBuf, image: String) -> Result<Self, E2eError> {
        if !executable.is_absolute() || image.trim().is_empty() {
            return Err(E2eError::InvalidRuntimeSpec);
        }
        let target = Self {
            executable,
            image,
            managed_containers: Arc::new(Mutex::new(Vec::new())),
        };
        target.run_checked(
            "daemon probe",
            &["version", "--format", "{{.Server.Version}}"],
        )?;
        target.run_checked("image probe", &["image", "inspect", target.image.as_str()])?;
        Ok(target)
    }

    fn run(&self, arguments: &[&str]) -> Result<Output, E2eError> {
        Command::new(&self.executable)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .map_err(E2eError::Io)
    }

    fn run_checked(&self, operation: &'static str, arguments: &[&str]) -> Result<String, E2eError> {
        let output = self.run(arguments)?;
        if !output.status.success() {
            return Err(E2eError::DockerCommand(operation));
        }
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_owned())
            .map_err(|_| E2eError::InvalidDockerOutput(operation))
    }

    fn container_name(installation_id: &InstallationId) -> String {
        format!("rumahl-e2e-{installation_id}")
    }

    fn spec_fingerprint(spec: &RuntimeInstallationSpec) -> Result<String, E2eError> {
        if spec.runtime().kind() != RuntimeKind::Container {
            return Err(E2eError::InvalidRuntimeSpec);
        }

        let mut artifact_count = 0;
        let mut fingerprint = format!(
            "{}|{}|{}|container",
            spec.identity().app_id(),
            spec.identity().publisher_id(),
            spec.version()
        );
        for entrypoint in spec.runtime().entrypoints() {
            match entrypoint.target() {
                RuntimeEntrypointTarget::ContainerArtifact(path) => {
                    artifact_count += 1;
                    if path.as_str() != EXPECTED_ARTIFACT {
                        return Err(E2eError::InvalidRuntimeSpec);
                    }
                    fingerprint.push_str(&format!(
                        "|{}:container-artifact:{}",
                        entrypoint.id(),
                        path
                    ));
                }
                RuntimeEntrypointTarget::Endpoint(endpoint) => {
                    fingerprint.push_str(&format!("|{}:endpoint:{}", entrypoint.id(), endpoint))
                }
                RuntimeEntrypointTarget::WebAsset(_) => {
                    return Err(E2eError::InvalidRuntimeSpec);
                }
            }
        }
        if artifact_count != 1 {
            return Err(E2eError::InvalidRuntimeSpec);
        }
        Ok(fingerprint)
    }

    fn find_container(&self, installation_id: &InstallationId) -> Result<Option<String>, E2eError> {
        let label = format!("label={INSTALLATION_LABEL}={installation_id}");
        let output = self.run_checked(
            "container lookup",
            &[
                "container",
                "ls",
                "--all",
                "--filter",
                &label,
                "--format",
                "{{.Names}}",
            ],
        )?;
        let names = output
            .lines()
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        match names.as_slice() {
            [] => Ok(None),
            [name] => Ok(Some((*name).to_owned())),
            _ => Err(E2eError::InvalidDockerOutput("container lookup")),
        }
    }

    fn container_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppRuntimeInstallationState, E2eError> {
        let Some(name) = self.find_container(installation_id)? else {
            return Ok(AppRuntimeInstallationState::Absent);
        };
        let running = self.run_checked(
            "container state",
            &[
                "container",
                "inspect",
                "--format",
                "{{.State.Running}}",
                &name,
            ],
        )?;
        match running.as_str() {
            "true" => Ok(AppRuntimeInstallationState::Active),
            "false" => Ok(AppRuntimeInstallationState::Prepared),
            _ => Err(E2eError::InvalidDockerOutput("container state")),
        }
    }

    fn fingerprint_for_container(&self, name: &str) -> Result<String, E2eError> {
        self.run_checked(
            "container fingerprint",
            &[
                "container",
                "inspect",
                "--format",
                "{{index .Config.Labels \"io.rumahl.e2e.spec\"}}",
                name,
            ],
        )
    }

    fn wait_until_ready(&self, installation_id: &InstallationId) -> Result<(), E2eError> {
        let name = Self::container_name(installation_id);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let output = self.run(&[
                "container",
                "exec",
                &name,
                "/bin/sh",
                "-c",
                "test -f /tmp/rumahl-ready",
            ])?;
            if output.status.success() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(E2eError::DockerCommand("container readiness"))
    }

    fn cleanup(&self) {
        let names = self
            .managed_containers
            .lock()
            .map(|names| names.clone())
            .unwrap_or_default();
        for name in names {
            let _ = self.run(&["container", "rm", "--force", &name]);
        }
    }
}

impl RuntimeControlTarget for DockerE2eTarget {
    type Error = E2eError;

    fn prepare(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        let expected_fingerprint = DockerE2eTarget::spec_fingerprint(&spec)?;
        let installation_id = spec.identity().installation_id();
        if let Some(name) = self.find_container(installation_id)? {
            if name != DockerE2eTarget::container_name(installation_id)
                || self.fingerprint_for_container(&name)? != expected_fingerprint
            {
                return Ok(RuntimeControlTargetOutcome::Conflict);
            }
            return Ok(RuntimeControlTargetOutcome::Accepted {
                state: self.container_state(installation_id)?,
                changed: false,
            });
        }

        let name = DockerE2eTarget::container_name(installation_id);
        let installation_label = format!("{INSTALLATION_LABEL}={installation_id}");
        let spec_label = format!("{SPEC_LABEL}={expected_fingerprint}");
        self.run_checked(
            "container create",
            &[
                "container",
                "create",
                "--name",
                &name,
                "--label",
                &installation_label,
                "--label",
                &spec_label,
                "--read-only",
                "--network",
                "none",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges=true",
                "--pids-limit",
                "64",
                "--memory",
                "64m",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,nodev,size=16m,mode=1777",
                "--user",
                "65534:65534",
                "--entrypoint",
                "/bin/sh",
                self.image.as_str(),
                "-c",
                "printf ready >/tmp/rumahl-ready; trap 'exit 0' TERM INT; while :; do sleep 1; done",
            ],
        )?;
        self.managed_containers.lock().unwrap().push(name);
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Prepared,
            changed: true,
        })
    }

    fn activate(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        let expected_fingerprint = DockerE2eTarget::spec_fingerprint(&spec)?;
        let installation_id = spec.identity().installation_id();
        let Some(name) = self.find_container(installation_id)? else {
            return Ok(RuntimeControlTargetOutcome::Rejected);
        };
        if self.fingerprint_for_container(&name)? != expected_fingerprint {
            return Ok(RuntimeControlTargetOutcome::Conflict);
        }
        if self.container_state(installation_id)? == AppRuntimeInstallationState::Active {
            return Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Active,
                changed: false,
            });
        }
        self.run_checked("container start", &["container", "start", &name])?;
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Active,
            changed: true,
        })
    }

    fn installation_state(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: self.container_state(&installation_id)?,
            changed: false,
        })
    }

    fn deactivate(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        let state = self.container_state(&installation_id)?;
        if state != AppRuntimeInstallationState::Active {
            return Ok(RuntimeControlTargetOutcome::Accepted {
                state,
                changed: false,
            });
        }
        let name = DockerE2eTarget::container_name(&installation_id);
        self.run_checked(
            "container stop",
            &["container", "stop", "--time", "2", &name],
        )?;
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Prepared,
            changed: true,
        })
    }

    fn remove(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        match self.container_state(&installation_id)? {
            AppRuntimeInstallationState::Absent => {
                return Ok(RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Absent,
                    changed: false,
                });
            }
            AppRuntimeInstallationState::Active => {
                return Ok(RuntimeControlTargetOutcome::Rejected);
            }
            AppRuntimeInstallationState::Prepared => {}
        }
        let name = DockerE2eTarget::container_name(&installation_id);
        self.run_checked("container remove", &["container", "rm", &name])?;
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Absent,
            changed: true,
        })
    }
}

struct Cleanup {
    root: PathBuf,
    target: DockerE2eTarget,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.target.cleanup();
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
    type Error = E2eError;

    fn resolve_origin(
        &self,
        _app: &InstalledApp,
        _entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error> {
        Err(E2eError::OriginResolutionWasUnexpected)
    }
}

struct NoRuntimeSecrets;

impl RuntimeSecretDelivery for NoRuntimeSecrets {
    type Error = E2eError;

    fn deliver_oidc_client_secret(
        &self,
        _operation_id: &rumahl_core::AppOperationId,
        _app: &InstalledApp,
        _client_id: &OidcClientId,
        _client_secret: &OidcClientSecret,
    ) -> Result<(), Self::Error> {
        Err(E2eError::OriginResolutionWasUnexpected)
    }

    fn remove_for_installation(
        &self,
        _operation_id: &rumahl_core::AppOperationId,
        _installation_id: &InstallationId,
    ) -> Result<(), Self::Error> {
        Err(E2eError::OriginResolutionWasUnexpected)
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
    let image = std::env::var(E2E_IMAGE_ENV)
        .unwrap_or_else(|_| panic!("{E2E_IMAGE_ENV} must name a pre-pulled immutable image"));
    let docker = std::env::var_os(E2E_DOCKER_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/docker"));
    let target = DockerE2eTarget::new(docker, image).unwrap();
    let root = test_root();
    let _cleanup = Cleanup {
        root: root.clone(),
        target: target.clone(),
    };
    let socket_path = root.join("runtime.sock");
    let server = UnixRuntimeControlServer::bind(
        UnixRuntimeControlServerConfig::new(&socket_path, current_uid()).unwrap(),
        target.clone(),
    )
    .unwrap();
    let server_thread = thread::spawn(move || {
        for _ in 0..4 {
            server.serve_once().unwrap();
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
    target
        .wait_until_ready(installed.installation_id())
        .unwrap();
    assert_state(
        target
            .installation_state(*installed.installation_id())
            .unwrap(),
        AppRuntimeInstallationState::Active,
    );
    assert_eq!(state.installed_apps().len(), 1);

    runner
        .uninstall(installed.installation_id(), &mut state, &mut grants)
        .unwrap();
    server_thread.join().unwrap();
    assert_state(
        target
            .installation_state(*installed.installation_id())
            .unwrap(),
        AppRuntimeInstallationState::Absent,
    );
    assert!(state.installed_apps().is_empty());
}
