use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rumahl_core::{
    AppRuntimeInstallationState, InstallationId, PackagePath, RuntimeEntrypointTarget, RuntimeKind,
};
use sha2::{Digest, Sha256};

use crate::{RuntimeControlTarget, RuntimeControlTargetOutcome, RuntimeInstallationSpec};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_STOP_TIMEOUT_SECONDS: u8 = 10;
const WAIT_INTERVAL: Duration = Duration::from_millis(10);
const MAX_COMMAND_OUTPUT: usize = 64 * 1024;
const CONTAINER_NAME_PREFIX: &str = "rumahl-app-";
const MANAGED_LABEL: &str = "com.rumahl.managed";
const INSTALLATION_LABEL: &str = "com.rumahl.installation";
const INSTANCE_LABEL: &str = "com.rumahl.supervisor";
const SPEC_LABEL: &str = "com.rumahl.runtime-spec";
const IMAGE_LABEL: &str = "com.rumahl.image-id";
const SECRET_MOUNT_PATH: &str = "/run/rumahl/secrets";
const SECRET_DIRECTORY: &str = "secrets";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerImageReference(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerImageReferenceError {
    Empty,
    TooLong,
    Mutable,
    InvalidCharacter,
    InvalidDigest,
}

pub trait DockerImageResolver {
    type Error: Error + Send + Sync + 'static;

    /// Resolves an already-imported package artifact to an immutable local image.
    ///
    /// Implementations own package verification and image import. The returned
    /// reference must contain a SHA-256 digest; tags are deliberately rejected.
    fn resolve_image(
        &self,
        spec: &RuntimeInstallationSpec,
        artifact: &PackagePath,
    ) -> Result<DockerImageReference, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct DockerRuntimeTargetConfig {
    executable: PathBuf,
    runtime_root: PathBuf,
    network: String,
    supervisor_instance: String,
    command_timeout: Duration,
    stop_timeout_seconds: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerRuntimeTargetConfigError {
    ExecutableMustBeAbsolute,
    RuntimeRootMustBeAbsolute,
    RuntimeRootCannotContainComma,
    InvalidNetwork,
    InvalidSupervisorInstance,
    ZeroCommandTimeout,
    InvalidStopTimeout,
}

#[derive(Debug, Clone)]
pub struct DockerRuntimeTarget<R> {
    config: DockerRuntimeTargetConfig,
    image_resolver: R,
}

#[derive(Debug)]
pub enum DockerRuntimeTargetError<E> {
    InvalidRuntimeSpec,
    ImageResolution(E),
    RuntimeRoot(io::Error),
    RuntimeRootIsNotDirectory,
    RuntimeRootOwnerMismatch {
        expected: u32,
        actual: u32,
    },
    RuntimeRootPermissions(u32),
    Namespace(io::Error),
    NamespaceIsNotDirectory,
    NamespaceOwnerMismatch {
        expected: u32,
        actual: u32,
    },
    NamespacePermissions(u32),
    Spawn(io::Error),
    Read(io::Error),
    Wait(io::Error),
    Terminate(io::Error),
    ReaderPanicked,
    TimedOut(&'static str),
    CommandFailed {
        operation: &'static str,
        status: Option<i32>,
    },
    OutputTooLarge(&'static str),
    InvalidOutput(&'static str),
}

#[derive(Debug)]
struct CommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    overflowed: bool,
}

impl DockerImageReference {
    pub const MAX_LENGTH: usize = 512;

    pub fn parse(value: impl Into<String>) -> Result<Self, DockerImageReferenceError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DockerImageReferenceError::Empty);
        }
        if value.len() > Self::MAX_LENGTH {
            return Err(DockerImageReferenceError::TooLong);
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/' | b':' | b'@')
        }) {
            return Err(DockerImageReferenceError::InvalidCharacter);
        }

        let digest = if let Some((name, digest)) = value.rsplit_once("@sha256:") {
            if name.is_empty() || name.contains('@') {
                return Err(DockerImageReferenceError::Mutable);
            }
            digest
        } else if let Some(digest) = value.strip_prefix("sha256:") {
            digest
        } else {
            return Err(DockerImageReferenceError::Mutable);
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(DockerImageReferenceError::InvalidDigest);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DockerImageReference {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for DockerImageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl DockerRuntimeTargetConfig {
    pub fn new(
        executable: impl Into<PathBuf>,
        runtime_root: impl Into<PathBuf>,
        network: impl Into<String>,
        supervisor_instance: impl Into<String>,
    ) -> Result<Self, DockerRuntimeTargetConfigError> {
        Self::with_timeouts(
            executable,
            runtime_root,
            network,
            supervisor_instance,
            DEFAULT_COMMAND_TIMEOUT,
            DEFAULT_STOP_TIMEOUT_SECONDS,
        )
    }

    pub fn with_timeouts(
        executable: impl Into<PathBuf>,
        runtime_root: impl Into<PathBuf>,
        network: impl Into<String>,
        supervisor_instance: impl Into<String>,
        command_timeout: Duration,
        stop_timeout_seconds: u8,
    ) -> Result<Self, DockerRuntimeTargetConfigError> {
        let executable = executable.into();
        let runtime_root = runtime_root.into();
        let network = network.into();
        let supervisor_instance = supervisor_instance.into();
        if !executable.is_absolute() {
            return Err(DockerRuntimeTargetConfigError::ExecutableMustBeAbsolute);
        }
        if !runtime_root.is_absolute() {
            return Err(DockerRuntimeTargetConfigError::RuntimeRootMustBeAbsolute);
        }
        if runtime_root
            .as_os_str()
            .to_str()
            .is_none_or(|value| value.contains(','))
        {
            return Err(DockerRuntimeTargetConfigError::RuntimeRootCannotContainComma);
        }
        if !valid_name(&network, 128) {
            return Err(DockerRuntimeTargetConfigError::InvalidNetwork);
        }
        if !valid_name(&supervisor_instance, 64) {
            return Err(DockerRuntimeTargetConfigError::InvalidSupervisorInstance);
        }
        if command_timeout.is_zero() {
            return Err(DockerRuntimeTargetConfigError::ZeroCommandTimeout);
        }
        if !(1..=60).contains(&stop_timeout_seconds) {
            return Err(DockerRuntimeTargetConfigError::InvalidStopTimeout);
        }

        Ok(Self {
            executable,
            runtime_root,
            network,
            supervisor_instance,
            command_timeout,
            stop_timeout_seconds,
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn runtime_root(&self) -> &Path {
        &self.runtime_root
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    pub fn supervisor_instance(&self) -> &str {
        &self.supervisor_instance
    }

    pub fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    pub fn stop_timeout_seconds(&self) -> u8 {
        self.stop_timeout_seconds
    }
}

impl<R> DockerRuntimeTarget<R>
where
    R: DockerImageResolver,
{
    pub fn new(config: DockerRuntimeTargetConfig, image_resolver: R) -> Self {
        Self {
            config,
            image_resolver,
        }
    }

    pub fn config(&self) -> &DockerRuntimeTargetConfig {
        &self.config
    }

    pub fn image_resolver(&self) -> &R {
        &self.image_resolver
    }

    pub fn probe(&self) -> Result<(), DockerRuntimeTargetError<R::Error>> {
        self.validate_runtime_root()?;
        self.run_checked(
            "daemon probe",
            ["version", "--format", "{{.Server.Version}}"],
        )?;
        self.run_checked(
            "network probe",
            [
                "network",
                "inspect",
                "--format",
                "{{.Id}}",
                self.config.network.as_str(),
            ],
        )?;
        Ok(())
    }

    fn run_checked<I, S>(
        &self,
        operation: &'static str,
        arguments: I,
    ) -> Result<String, DockerRuntimeTargetError<R::Error>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = run_command(
            &self.config.executable,
            arguments,
            self.config.command_timeout,
            operation,
        )?;
        if !output.status.success() {
            return Err(DockerRuntimeTargetError::CommandFailed {
                operation,
                status: output.status.code(),
            });
        }
        if output.overflowed {
            return Err(DockerRuntimeTargetError::OutputTooLarge(operation));
        }
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_owned())
            .map_err(|_| DockerRuntimeTargetError::InvalidOutput(operation))
    }

    fn validate_runtime_root(&self) -> Result<(), DockerRuntimeTargetError<R::Error>> {
        let metadata = fs::symlink_metadata(&self.config.runtime_root)
            .map_err(DockerRuntimeTargetError::RuntimeRoot)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(DockerRuntimeTargetError::RuntimeRootIsNotDirectory);
        }
        let expected = current_uid();
        if metadata.uid() != expected {
            return Err(DockerRuntimeTargetError::RuntimeRootOwnerMismatch {
                expected,
                actual: metadata.uid(),
            });
        }
        let mode = metadata.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(DockerRuntimeTargetError::RuntimeRootPermissions(mode));
        }
        Ok(())
    }

    fn ensure_namespace(
        &self,
        installation_id: &InstallationId,
    ) -> Result<(PathBuf, bool), DockerRuntimeTargetError<R::Error>> {
        self.validate_runtime_root()?;
        let path = self.config.runtime_root.join(installation_id.to_string());
        let created = match fs::create_dir(&path) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(DockerRuntimeTargetError::Namespace(error)),
        };
        let metadata = fs::symlink_metadata(&path).map_err(DockerRuntimeTargetError::Namespace)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(DockerRuntimeTargetError::NamespaceIsNotDirectory);
        }
        let expected = current_uid();
        if metadata.uid() != expected {
            return Err(DockerRuntimeTargetError::NamespaceOwnerMismatch {
                expected,
                actual: metadata.uid(),
            });
        }
        let mode = metadata.mode() & 0o777;
        if !created && mode & 0o022 != 0 {
            return Err(DockerRuntimeTargetError::NamespacePermissions(mode));
        }
        if created {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(DockerRuntimeTargetError::Namespace)?;
        }
        let secret_path = path.join(SECRET_DIRECTORY);
        match fs::create_dir(&secret_path) {
            Ok(()) => fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o755))
                .map_err(DockerRuntimeTargetError::Namespace)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&secret_path)
                    .map_err(DockerRuntimeTargetError::Namespace)?;
                if !metadata.file_type().is_dir()
                    || metadata.file_type().is_symlink()
                    || metadata.uid() != expected
                    || metadata.mode() & 0o022 != 0
                {
                    return Err(DockerRuntimeTargetError::NamespaceIsNotDirectory);
                }
            }
            Err(error) => return Err(DockerRuntimeTargetError::Namespace(error)),
        }
        Ok((path, created))
    }

    fn find_container(
        &self,
        installation_id: &InstallationId,
    ) -> Result<Option<String>, DockerRuntimeTargetError<R::Error>> {
        let expected_name = container_name(installation_id);
        let name_filter = format!("name=^/{expected_name}$");
        let output = self.run_checked(
            "container lookup",
            [
                "container",
                "ls",
                "--all",
                "--filter",
                name_filter.as_str(),
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
            [name] if *name == expected_name => Ok(Some((*name).to_owned())),
            _ => Err(DockerRuntimeTargetError::InvalidOutput("container lookup")),
        }
    }

    fn inspect_label(
        &self,
        name: &str,
        label: &'static str,
    ) -> Result<String, DockerRuntimeTargetError<R::Error>> {
        let format = format!("{{{{index .Config.Labels \"{label}\"}}}}");
        self.run_checked(
            "container label inspection",
            ["container", "inspect", "--format", format.as_str(), name],
        )
    }

    fn container_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppRuntimeInstallationState, DockerRuntimeTargetError<R::Error>> {
        let Some(name) = self.find_container(installation_id)? else {
            return Ok(AppRuntimeInstallationState::Absent);
        };
        if !self.container_is_owned(&name, installation_id)? {
            return Err(DockerRuntimeTargetError::InvalidOutput(
                "container ownership",
            ));
        }
        let state = self.run_checked(
            "container state",
            [
                "container",
                "inspect",
                "--format",
                "{{.State.Status}}",
                name.as_str(),
            ],
        )?;
        match state.as_str() {
            "running" | "paused" | "restarting" => Ok(AppRuntimeInstallationState::Active),
            "created" | "exited" => Ok(AppRuntimeInstallationState::Prepared),
            _ => Err(DockerRuntimeTargetError::InvalidOutput("container state")),
        }
    }

    fn matching_container(
        &self,
        installation_id: &InstallationId,
        spec_fingerprint: &str,
    ) -> Result<Option<String>, DockerRuntimeTargetError<R::Error>> {
        let Some(name) = self.find_container(installation_id)? else {
            return Ok(None);
        };
        let configured_image = self.run_checked(
            "container image inspection",
            [
                "container",
                "inspect",
                "--format",
                "{{.Image}}",
                name.as_str(),
            ],
        )?;
        if !self.container_is_owned(&name, installation_id)?
            || self.inspect_label(&name, SPEC_LABEL)? != spec_fingerprint
            || self.inspect_label(&name, IMAGE_LABEL)? != configured_image
        {
            return Ok(Some(String::new()));
        }
        Ok(Some(name))
    }

    fn container_is_owned(
        &self,
        name: &str,
        installation_id: &InstallationId,
    ) -> Result<bool, DockerRuntimeTargetError<R::Error>> {
        Ok(self.inspect_label(name, MANAGED_LABEL)? == "true"
            && self.inspect_label(name, INSTALLATION_LABEL)? == installation_id.to_string()
            && self.inspect_label(name, INSTANCE_LABEL)? == self.config.supervisor_instance)
    }
}

impl<R> RuntimeControlTarget for DockerRuntimeTarget<R>
where
    R: DockerImageResolver,
{
    type Error = DockerRuntimeTargetError<R::Error>;

    fn prepare(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        let (fingerprint, artifact) = spec_fingerprint_and_artifact(&spec)?;
        let installation_id = spec.identity().installation_id();
        if let Some(name) = self.matching_container(installation_id, &fingerprint)? {
            if name.is_empty() {
                return Ok(RuntimeControlTargetOutcome::Conflict);
            }
            return Ok(RuntimeControlTargetOutcome::Accepted {
                state: self.container_state(installation_id)?,
                changed: false,
            });
        }

        let image = self
            .image_resolver
            .resolve_image(&spec, artifact)
            .map_err(DockerRuntimeTargetError::ImageResolution)?;
        let image_id = self.run_checked(
            "image inspection",
            ["image", "inspect", "--format", "{{.Id}}", image.as_str()],
        )?;
        validate_image_id(&image_id)?;

        let (namespace, namespace_created) = self.ensure_namespace(installation_id)?;
        let name = container_name(installation_id);
        let managed_label = format!("{MANAGED_LABEL}=true");
        let installation_label = format!("{INSTALLATION_LABEL}={installation_id}");
        let instance_label = format!("{INSTANCE_LABEL}={}", self.config.supervisor_instance);
        let spec_label = format!("{SPEC_LABEL}={fingerprint}");
        let image_label = format!("{IMAGE_LABEL}={image_id}");
        let mount = format!(
            "type=bind,src={},dst={SECRET_MOUNT_PATH},readonly,bind-propagation=rprivate",
            namespace.join(SECRET_DIRECTORY).display()
        );
        let stop_timeout = self.config.stop_timeout_seconds.to_string();
        let result = self.run_checked(
            "container create",
            [
                "container",
                "create",
                "--name",
                name.as_str(),
                "--label",
                managed_label.as_str(),
                "--label",
                installation_label.as_str(),
                "--label",
                instance_label.as_str(),
                "--label",
                spec_label.as_str(),
                "--label",
                image_label.as_str(),
                "--read-only",
                "--network",
                self.config.network.as_str(),
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges=true",
                "--pids-limit",
                "128",
                "--memory",
                "256m",
                "--stop-timeout",
                stop_timeout.as_str(),
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,nodev,size=32m,mode=1777",
                "--user",
                "65534:65534",
                "--mount",
                mount.as_str(),
                image.as_str(),
            ],
        );
        if result.is_err() && namespace_created {
            let _ = fs::remove_dir(namespace.join(SECRET_DIRECTORY));
            let _ = fs::remove_dir(&namespace);
        }
        result?;

        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Prepared,
            changed: true,
        })
    }

    fn activate(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
        let (fingerprint, _) = spec_fingerprint_and_artifact(&spec)?;
        let installation_id = spec.identity().installation_id();
        let Some(name) = self.matching_container(installation_id, &fingerprint)? else {
            return Ok(RuntimeControlTargetOutcome::Rejected);
        };
        if name.is_empty() {
            return Ok(RuntimeControlTargetOutcome::Conflict);
        }
        if self.container_state(installation_id)? == AppRuntimeInstallationState::Active {
            return Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Active,
                changed: false,
            });
        }
        self.run_checked("container start", ["container", "start", name.as_str()])?;
        if self.container_state(installation_id)? != AppRuntimeInstallationState::Active {
            return Ok(RuntimeControlTargetOutcome::Rejected);
        }
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
        let name = container_name(&installation_id);
        let timeout = self.config.stop_timeout_seconds.to_string();
        self.run_checked(
            "container stop",
            [
                "container",
                "stop",
                "--time",
                timeout.as_str(),
                name.as_str(),
            ],
        )?;
        if self.container_state(&installation_id)? == AppRuntimeInstallationState::Active {
            return Ok(RuntimeControlTargetOutcome::Rejected);
        }
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
                remove_namespace_if_empty::<R::Error>(
                    &self.config.runtime_root.join(installation_id.to_string()),
                )?;
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
        let name = container_name(&installation_id);
        self.run_checked("container remove", ["container", "rm", name.as_str()])?;
        remove_namespace_if_empty::<R::Error>(
            &self.config.runtime_root.join(installation_id.to_string()),
        )?;
        Ok(RuntimeControlTargetOutcome::Accepted {
            state: AppRuntimeInstallationState::Absent,
            changed: true,
        })
    }
}

fn run_command<I, S, E>(
    executable: &Path,
    arguments: I,
    timeout: Duration,
    operation: &'static str,
) -> Result<CommandOutput, DockerRuntimeTargetError<E>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(executable)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(DockerRuntimeTargetError::Spawn)?;
    let mut stdout = child.stdout.take().expect("stdout is configured as piped");
    let reader = thread::spawn(move || {
        let mut captured = Vec::new();
        let mut overflowed = false;
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stdout.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let remaining = MAX_COMMAND_OUTPUT.saturating_sub(captured.len());
            let retained = remaining.min(read);
            captured.extend_from_slice(&buffer[..retained]);
            overflowed |= retained != read;
        }
        Ok::<_, io::Error>((captured, overflowed))
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(WAIT_INTERVAL),
            Ok(None) => {
                let terminate_result = child.kill();
                let wait_result = child.wait();
                let reader_result = join_reader(reader);
                terminate_result.map_err(DockerRuntimeTargetError::Terminate)?;
                wait_result.map_err(DockerRuntimeTargetError::Wait)?;
                drop(reader_result?);
                return Err(DockerRuntimeTargetError::TimedOut(operation));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                drop(join_reader(reader)?);
                return Err(DockerRuntimeTargetError::Wait(error));
            }
        }
    };
    let (stdout, overflowed) = join_reader(reader)?;
    Ok(CommandOutput {
        status,
        stdout,
        overflowed,
    })
}

fn join_reader<E>(
    reader: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>,
) -> Result<(Vec<u8>, bool), DockerRuntimeTargetError<E>> {
    reader
        .join()
        .map_err(|_| DockerRuntimeTargetError::ReaderPanicked)?
        .map_err(DockerRuntimeTargetError::Read)
}

fn valid_name(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn current_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions.
    unsafe { libc::geteuid() }
}

fn container_name(installation_id: &InstallationId) -> String {
    format!("{CONTAINER_NAME_PREFIX}{installation_id}")
}

fn spec_fingerprint_and_artifact<E>(
    spec: &RuntimeInstallationSpec,
) -> Result<(String, &PackagePath), DockerRuntimeTargetError<E>> {
    if spec.runtime().kind() != RuntimeKind::Container {
        return Err(DockerRuntimeTargetError::InvalidRuntimeSpec);
    }
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, spec.identity().installation_id().to_string());
    hash_field(&mut hasher, spec.identity().app_id().as_str());
    hash_field(&mut hasher, spec.identity().publisher_id().as_str());
    hash_field(&mut hasher, spec.version().to_string());
    hash_field(&mut hasher, "container");
    let mut artifact = None;
    for entrypoint in spec.runtime().entrypoints() {
        hash_field(&mut hasher, entrypoint.id().as_str());
        match entrypoint.target() {
            RuntimeEntrypointTarget::ContainerArtifact(path) => {
                if artifact.replace(path).is_some() {
                    return Err(DockerRuntimeTargetError::InvalidRuntimeSpec);
                }
                hash_field(&mut hasher, "container-artifact");
                hash_field(&mut hasher, path.as_str());
            }
            RuntimeEntrypointTarget::Endpoint(endpoint) => {
                hash_field(&mut hasher, "endpoint");
                hash_field(&mut hasher, endpoint.as_str());
            }
            RuntimeEntrypointTarget::WebAsset(_) => {
                return Err(DockerRuntimeTargetError::InvalidRuntimeSpec);
            }
        }
    }
    let artifact = artifact.ok_or(DockerRuntimeTargetError::InvalidRuntimeSpec)?;
    Ok((hex(&hasher.finalize()), artifact))
}

fn hash_field(hasher: &mut Sha256, value: impl AsRef<[u8]>) {
    let value = value.as_ref();
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn validate_image_id<E>(value: &str) -> Result<(), DockerRuntimeTargetError<E>> {
    DockerImageReference::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| DockerRuntimeTargetError::InvalidOutput("image inspection"))
}

fn remove_namespace_if_empty<E>(path: &Path) -> Result<(), DockerRuntimeTargetError<E>> {
    match fs::remove_dir(path.join(SECRET_DIRECTORY)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(DockerRuntimeTargetError::Namespace(error)),
    }
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DockerRuntimeTargetError::Namespace(error)),
    }
}

impl fmt::Display for DockerImageReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Docker image reference must use an immutable SHA-256 digest"
        )
    }
}

impl Error for DockerImageReferenceError {}

impl fmt::Display for DockerRuntimeTargetConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutableMustBeAbsolute => write!(f, "Docker executable path must be absolute"),
            Self::RuntimeRootMustBeAbsolute => write!(f, "runtime root path must be absolute"),
            Self::RuntimeRootCannotContainComma => {
                write!(
                    f,
                    "runtime root path must be UTF-8 and cannot contain a comma"
                )
            }
            Self::InvalidNetwork => write!(f, "Docker network name is invalid"),
            Self::InvalidSupervisorInstance => write!(f, "supervisor instance name is invalid"),
            Self::ZeroCommandTimeout => write!(f, "Docker command timeout must be non-zero"),
            Self::InvalidStopTimeout => write!(f, "Docker stop timeout must be 1 to 60 seconds"),
        }
    }
}

impl Error for DockerRuntimeTargetConfigError {}

impl<E> fmt::Display for DockerRuntimeTargetError<E>
where
    E: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuntimeSpec => write!(f, "runtime specification is not a container app"),
            Self::ImageResolution(_) => write!(f, "container image resolution failed"),
            Self::RuntimeRoot(_) => write!(f, "runtime root inspection failed"),
            Self::RuntimeRootIsNotDirectory => {
                write!(f, "runtime root is not a non-symlink directory")
            }
            Self::RuntimeRootOwnerMismatch { expected, actual } => write!(
                f,
                "runtime root owner UID {actual} does not match supervisor UID {expected}"
            ),
            Self::RuntimeRootPermissions(mode) => {
                write!(
                    f,
                    "runtime root permissions {mode:o} allow non-owner writes"
                )
            }
            Self::Namespace(_) => write!(f, "runtime namespace operation failed"),
            Self::NamespaceIsNotDirectory => {
                write!(f, "runtime namespace is not a non-symlink directory")
            }
            Self::NamespaceOwnerMismatch { expected, actual } => write!(
                f,
                "runtime namespace owner UID {actual} does not match supervisor UID {expected}"
            ),
            Self::NamespacePermissions(mode) => write!(
                f,
                "runtime namespace permissions {mode:o} allow non-owner writes"
            ),
            Self::Spawn(_) => write!(f, "failed to execute Docker CLI"),
            Self::Read(_) => write!(f, "failed to read Docker CLI output"),
            Self::Wait(_) => write!(f, "failed while waiting for Docker CLI"),
            Self::Terminate(_) => write!(f, "failed to terminate Docker CLI"),
            Self::ReaderPanicked => write!(f, "Docker CLI output reader failed"),
            Self::TimedOut(operation) => write!(f, "Docker operation '{operation}' timed out"),
            Self::CommandFailed { operation, status } => {
                write!(
                    f,
                    "Docker operation '{operation}' failed with status {status:?}"
                )
            }
            Self::OutputTooLarge(operation) => {
                write!(f, "Docker operation '{operation}' returned too much output")
            }
            Self::InvalidOutput(operation) => {
                write!(f, "Docker operation '{operation}' returned invalid output")
            }
        }
    }
}

impl<E> Error for DockerRuntimeTargetError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ImageResolution(error) => Some(error),
            Self::RuntimeRoot(error)
            | Self::Namespace(error)
            | Self::Spawn(error)
            | Self::Read(error)
            | Self::Wait(error)
            | Self::Terminate(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::test_support::unique_test_root;
    use rumahl_core::{
        AppId, AppIdentity, AppVersion, InstallationId, PublisherId, RuntimeDescriptor,
        RuntimeEntrypoint, RuntimeEntrypointId,
    };

    struct UnusedImageResolver;

    impl DockerImageResolver for UnusedImageResolver {
        type Error = Infallible;

        fn resolve_image(
            &self,
            _spec: &RuntimeInstallationSpec,
            _artifact: &PackagePath,
        ) -> Result<DockerImageReference, Self::Error> {
            unreachable!("namespace tests do not resolve images")
        }
    }

    fn container_spec(version: AppVersion, artifact: &str) -> RuntimeInstallationSpec {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse(artifact).unwrap(),
            ))
            .unwrap();
        RuntimeInstallationSpec::new(
            AppIdentity::new(
                AppId::parse("com.rumahl.test").unwrap(),
                InstallationId::new(),
                PublisherId::parse("com.rumahl").unwrap(),
            ),
            version,
            runtime,
        )
    }

    #[test]
    fn accepts_only_immutable_sha256_image_references() {
        let digest = "a".repeat(64);
        assert!(DockerImageReference::parse(format!("registry.local/app@sha256:{digest}")).is_ok());
        assert!(DockerImageReference::parse(format!("sha256:{digest}")).is_ok());
        assert_eq!(
            DockerImageReference::parse("registry.local/app:latest").unwrap_err(),
            DockerImageReferenceError::Mutable
        );
        assert_eq!(
            DockerImageReference::parse(format!("registry.local/app@sha256:{}", "A".repeat(64)))
                .unwrap_err(),
            DockerImageReferenceError::InvalidDigest
        );
    }

    #[test]
    fn validates_trusted_docker_configuration() {
        assert_eq!(
            DockerRuntimeTargetConfig::new("docker", "/run/rumahl/apps", "rumahl-apps", "system")
                .unwrap_err(),
            DockerRuntimeTargetConfigError::ExecutableMustBeAbsolute
        );
        assert_eq!(
            DockerRuntimeTargetConfig::new("/usr/bin/docker", "relative", "rumahl-apps", "system")
                .unwrap_err(),
            DockerRuntimeTargetConfigError::RuntimeRootMustBeAbsolute
        );
        assert_eq!(
            DockerRuntimeTargetConfig::new(
                "/usr/bin/docker",
                "/run/rumahl/apps",
                "host,other",
                "system"
            )
            .unwrap_err(),
            DockerRuntimeTargetConfigError::InvalidNetwork
        );
    }

    #[test]
    fn fingerprints_complete_runtime_specification() {
        let original = container_spec(AppVersion::new(1, 0, 0), "runtime/server.oci");
        let changed_version = RuntimeInstallationSpec::new(
            original.identity().clone(),
            AppVersion::new(1, 0, 1),
            original.runtime().clone(),
        );
        let changed_artifact = RuntimeInstallationSpec::new(
            original.identity().clone(),
            original.version(),
            container_spec(AppVersion::new(1, 0, 0), "runtime/other.oci")
                .runtime()
                .clone(),
        );

        let original_fingerprint = spec_fingerprint_and_artifact::<Infallible>(&original)
            .unwrap()
            .0;
        assert_eq!(
            original_fingerprint,
            spec_fingerprint_and_artifact::<Infallible>(&original)
                .unwrap()
                .0
        );
        assert_ne!(
            original_fingerprint,
            spec_fingerprint_and_artifact::<Infallible>(&changed_version)
                .unwrap()
                .0
        );
        assert_ne!(
            original_fingerprint,
            spec_fingerprint_and_artifact::<Infallible>(&changed_artifact)
                .unwrap()
                .0
        );
    }

    #[test]
    fn creates_private_secret_namespace_and_rejects_insecure_replay() {
        let root = unique_test_root('m');
        let target = DockerRuntimeTarget::new(
            DockerRuntimeTargetConfig::new("/usr/bin/docker", &root, "rumahl-apps", "system")
                .unwrap(),
            UnusedImageResolver,
        );
        let installation_id = InstallationId::new();
        let (namespace, created) = target.ensure_namespace(&installation_id).unwrap();
        assert!(created);
        assert_eq!(
            fs::metadata(&namespace).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(namespace.join(SECRET_DIRECTORY))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );

        fs::set_permissions(&namespace, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(matches!(
            target.ensure_namespace(&installation_id),
            Err(DockerRuntimeTargetError::NamespacePermissions(0o770))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn terminates_a_hung_docker_command() {
        let root = unique_test_root('d');
        let executable = root.join("docker");
        fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 5\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

        let started = Instant::now();
        let error = run_command::<_, _, Infallible>(
            &executable,
            std::iter::empty::<&str>(),
            Duration::from_millis(30),
            "test probe",
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DockerRuntimeTargetError::TimedOut("test probe")
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        fs::remove_dir_all(root).unwrap();
    }
}
