use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rumahl_core::{InstallationId, PackagePath};

use crate::{
    DockerImageReference, DockerImageReferenceError, StagedDockerImageWriter,
    StagedDockerImageWriterError,
};

const DEFAULT_IMPORT_TIMEOUT: Duration = Duration::from_secs(120);
const WAIT_INTERVAL: Duration = Duration::from_millis(10);
const MAX_COMMAND_OUTPUT: usize = 256 * 1024;
const SHA256_PREFIX: &str = "sha256:";
const LOADED_IMAGE_PREFIX: &str = "Loaded image: ";
const ARCHIVE_FILE_MODE: u32 = 0o600;

/// Upper bound for one imported image archive.
pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

/// Stages the declared OCI/Docker artifact of an installation so the runtime
/// resolver can hand the supervisor an immutable image reference.
pub trait ContainerImageImporter {
    type Error: Error + Send + Sync + 'static;

    fn stage_image(
        &self,
        installation_id: &InstallationId,
        package_root: &Path,
        artifact: &PackagePath,
    ) -> Result<(), Self::Error>;
}

/// Host-side boundary for loading an image archive into the Docker engine and
/// recording the immutable reference for an installation.
pub trait DockerImageTarget {
    type Error: Error + Send + Sync + 'static;

    fn import_archive(
        &self,
        installation_id: &InstallationId,
        artifact: &PackagePath,
        archive: &[u8],
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone)]
pub struct DockerImageImporterConfig {
    executable: PathBuf,
    work_root: PathBuf,
    image_root: PathBuf,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerImageImporterConfigError {
    ExecutableMustBeAbsolute,
    WorkRootMustBeAbsolute,
    ImageRootMustBeAbsolute,
    ZeroTimeout,
}

/// Imports a package's OCI/Docker archive through a fixed Docker executable.
///
/// The process runs without a shell, with a cleared environment and bounded
/// output/time. It lives on the supervisor side of the trust boundary: the
/// platform service deliberately has no Docker access.
#[derive(Debug)]
pub struct DockerImageImporter {
    config: DockerImageImporterConfig,
    writer: StagedDockerImageWriter,
}

#[derive(Debug)]
pub enum DockerImageImportError {
    ArtifactSymlink,
    ArtifactNotRegular,
    ArtifactTooLarge,
    ArtifactRead(io::Error),
    ArchiveEmpty,
    ArchiveTooLarge,
    WorkFile(io::Error),
    ArchiveWrite(io::Error),
    Spawn(io::Error),
    Read(io::Error),
    Wait(io::Error),
    Terminate(io::Error),
    ReaderPanicked,
    TimedOut(&'static str),
    OutputTooLarge(&'static str),
    LoadFailed(Option<i32>),
    InspectFailed(Option<i32>),
    UnparsableOutput,
    AmbiguousImage,
    InvalidReference(DockerImageReferenceError),
    StagedReference(StagedDockerImageWriterError),
}

#[derive(Debug)]
struct CommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl DockerImageImporterConfig {
    pub fn new(
        executable: impl Into<PathBuf>,
        work_root: impl Into<PathBuf>,
        image_root: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Result<Self, DockerImageImporterConfigError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(DockerImageImporterConfigError::ExecutableMustBeAbsolute);
        }
        let work_root = work_root.into();
        if !work_root.is_absolute() {
            return Err(DockerImageImporterConfigError::WorkRootMustBeAbsolute);
        }
        let image_root = image_root.into();
        if !image_root.is_absolute() {
            return Err(DockerImageImporterConfigError::ImageRootMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(DockerImageImporterConfigError::ZeroTimeout);
        }
        Ok(Self {
            executable,
            work_root,
            image_root,
            timeout,
        })
    }

    pub fn with_default_timeout(
        executable: impl Into<PathBuf>,
        work_root: impl Into<PathBuf>,
        image_root: impl Into<PathBuf>,
    ) -> Result<Self, DockerImageImporterConfigError> {
        Self::new(executable, work_root, image_root, DEFAULT_IMPORT_TIMEOUT)
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn work_root(&self) -> &Path {
        &self.work_root
    }

    pub fn image_root(&self) -> &Path {
        &self.image_root
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl DockerImageImporter {
    pub fn new(config: DockerImageImporterConfig) -> Self {
        let writer = StagedDockerImageWriter::new(config.image_root.clone())
            .expect("image root was validated as absolute");
        Self { config, writer }
    }

    pub fn config(&self) -> &DockerImageImporterConfig {
        &self.config
    }

    fn load_archive(&self, path: &Path) -> Result<DockerImageReference, DockerImageImportError> {
        let output = run_command(
            &self.config.executable,
            &[
                "load".to_owned(),
                "--input".to_owned(),
                path.as_os_str().to_string_lossy().into_owned(),
            ],
            self.config.timeout,
            "image load",
        )?;
        if !output.status.success() {
            return Err(DockerImageImportError::LoadFailed(output.status.code()));
        }

        let text = combined_output(&output);
        if let Some(digest) = unique_token(&text) {
            return self.reference(digest);
        }

        let names = loaded_image_names(&text);
        match names.as_slice() {
            [name] => self.inspect_image_id(name),
            [] => Err(DockerImageImportError::UnparsableOutput),
            _ => Err(DockerImageImportError::AmbiguousImage),
        }
    }

    fn inspect_image_id(
        &self,
        reference: &str,
    ) -> Result<DockerImageReference, DockerImageImportError> {
        let output = run_command(
            &self.config.executable,
            &[
                "image".to_owned(),
                "inspect".to_owned(),
                "--format".to_owned(),
                "{{.Id}}".to_owned(),
                reference.to_owned(),
            ],
            self.config.timeout,
            "image inspection",
        )?;
        if !output.status.success() {
            return Err(DockerImageImportError::InspectFailed(output.status.code()));
        }
        let text = combined_output(&output);
        match unique_token(&text) {
            Some(digest) => self.reference(digest),
            None => Err(DockerImageImportError::UnparsableOutput),
        }
    }

    fn reference(&self, digest: String) -> Result<DockerImageReference, DockerImageImportError> {
        DockerImageReference::parse(format!("{SHA256_PREFIX}{digest}"))
            .map_err(DockerImageImportError::InvalidReference)
    }
}

impl DockerImageTarget for DockerImageImporter {
    type Error = DockerImageImportError;

    fn import_archive(
        &self,
        installation_id: &InstallationId,
        artifact: &PackagePath,
        archive: &[u8],
    ) -> Result<(), Self::Error> {
        if archive.is_empty() {
            return Err(DockerImageImportError::ArchiveEmpty);
        }
        if archive.len() > MAX_ARCHIVE_BYTES {
            return Err(DockerImageImportError::ArchiveTooLarge);
        }

        let path = self
            .config
            .work_root
            .join(format!(".image-load-{}", InstallationId::new()));
        {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(ARCHIVE_FILE_MODE)
                .open(&path)
                .map_err(DockerImageImportError::WorkFile)?;
            file.write_all(archive)
                .map_err(DockerImageImportError::ArchiveWrite)?;
            file.sync_all()
                .map_err(DockerImageImportError::ArchiveWrite)?;
        }

        let outcome = self.load_archive(&path).and_then(|reference| {
            self.writer
                .write(installation_id, artifact, &reference)
                .map_err(DockerImageImportError::StagedReference)
        });
        let _ = fs::remove_file(&path);
        outcome
    }
}

impl ContainerImageImporter for DockerImageImporter {
    type Error = DockerImageImportError;

    fn stage_image(
        &self,
        installation_id: &InstallationId,
        package_root: &Path,
        artifact: &PackagePath,
    ) -> Result<(), Self::Error> {
        let path = package_root.join(artifact.as_str());
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| DockerImageImportError::ArtifactNotRegular)?;
        if metadata.file_type().is_symlink() {
            return Err(DockerImageImportError::ArtifactSymlink);
        }
        if !metadata.file_type().is_file() {
            return Err(DockerImageImportError::ArtifactNotRegular);
        }
        if metadata.len() > MAX_ARCHIVE_BYTES as u64 {
            return Err(DockerImageImportError::ArtifactTooLarge);
        }

        let archive = fs::read(&path).map_err(DockerImageImportError::ArtifactRead)?;
        self.import_archive(installation_id, artifact, &archive)
    }
}

fn combined_output(output: &CommandOutput) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

fn run_command(
    executable: &Path,
    arguments: &[String],
    timeout: Duration,
    operation: &'static str,
) -> Result<CommandOutput, DockerImageImportError> {
    let mut child = Command::new(executable)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(DockerImageImportError::Spawn)?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let stdout_reader = spawn_reader(stdout);
    let stderr_reader = spawn_reader(stderr);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(WAIT_INTERVAL),
            Ok(None) => {
                let terminate = child.kill();
                let wait = child.wait();
                drop(join_reader(stdout_reader)?);
                drop(join_reader(stderr_reader)?);
                terminate.map_err(DockerImageImportError::Terminate)?;
                wait.map_err(DockerImageImportError::Wait)?;
                return Err(DockerImageImportError::TimedOut(operation));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                drop(join_reader(stdout_reader)?);
                drop(join_reader(stderr_reader)?);
                return Err(DockerImageImportError::Wait(error));
            }
        }
    };

    let (stdout, stdout_overflowed) = join_reader(stdout_reader)?;
    let (stderr, stderr_overflowed) = join_reader(stderr_reader)?;
    if stdout_overflowed || stderr_overflowed {
        return Err(DockerImageImportError::OutputTooLarge(operation));
    }

    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

type ReaderResult = io::Result<(Vec<u8>, bool)>;

fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> thread::JoinHandle<ReaderResult> {
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut overflowed = false;
        let mut buffer = [0_u8; 4096];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let remaining = MAX_COMMAND_OUTPUT.saturating_sub(captured.len());
            let retained = remaining.min(read);
            captured.extend_from_slice(&buffer[..retained]);
            overflowed |= retained != read;
        }
        Ok((captured, overflowed))
    })
}

fn join_reader(
    reader: thread::JoinHandle<ReaderResult>,
) -> Result<(Vec<u8>, bool), DockerImageImportError> {
    reader
        .join()
        .map_err(|_| DockerImageImportError::ReaderPanicked)?
        .map_err(DockerImageImportError::Read)
}

/// Returns the single SHA-256 token in `text`, if the text contains exactly one
/// distinct digest.
fn unique_token(text: &str) -> Option<String> {
    let mut tokens = sha256_tokens(text);
    tokens.sort();
    tokens.dedup();
    match tokens.as_slice() {
        [token] => Some(token.clone()),
        _ => None,
    }
}

fn sha256_tokens(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index + SHA256_PREFIX.len() <= bytes.len() {
        if bytes[index..].starts_with(SHA256_PREFIX.as_bytes()) {
            let start = index + SHA256_PREFIX.len();
            if start + 64 <= bytes.len()
                && bytes[start..start + 64]
                    .iter()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            {
                tokens.push(text[start..start + 64].to_owned());
                index = start + 64;
                continue;
            }
        }
        index += 1;
    }
    tokens
}

fn loaded_image_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix(LOADED_IMAGE_PREFIX) {
            let name = name.trim().to_owned();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

impl fmt::Display for DockerImageImporterConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutableMustBeAbsolute => write!(f, "docker executable path must be absolute"),
            Self::WorkRootMustBeAbsolute => write!(f, "docker import work root must be absolute"),
            Self::ImageRootMustBeAbsolute => write!(f, "staged image root must be absolute"),
            Self::ZeroTimeout => write!(f, "docker import timeout must be non-zero"),
        }
    }
}

impl Error for DockerImageImporterConfigError {}

impl fmt::Display for DockerImageImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactSymlink => write!(f, "package image artifact must not be a symlink"),
            Self::ArtifactNotRegular => {
                write!(f, "package image artifact is not a regular file")
            }
            Self::ArtifactTooLarge => write!(f, "package image artifact exceeds the size limit"),
            Self::ArtifactRead(_) => write!(f, "package image artifact could not be read"),
            Self::ArchiveEmpty => write!(f, "image archive is empty"),
            Self::ArchiveTooLarge => write!(f, "image archive exceeds the size limit"),
            Self::WorkFile(_) => write!(f, "image archive work file could not be created"),
            Self::ArchiveWrite(_) => write!(f, "image archive work file could not be written"),
            Self::Spawn(_) => write!(f, "docker image load could not start"),
            Self::Read(_) => write!(f, "docker image load output could not be read"),
            Self::Wait(_) => write!(f, "docker image load could not be awaited"),
            Self::Terminate(_) => write!(f, "docker image load could not be terminated"),
            Self::ReaderPanicked => write!(f, "docker image load output reader failed"),
            Self::TimedOut(operation) => write!(f, "docker {operation} did not finish in time"),
            Self::OutputTooLarge(operation) => {
                write!(f, "docker {operation} produced too much output")
            }
            Self::LoadFailed(code) => write!(f, "docker image load failed with {code:?}"),
            Self::InspectFailed(code) => write!(f, "docker image inspection failed with {code:?}"),
            Self::UnparsableOutput => write!(f, "docker image load output is not parsable"),
            Self::AmbiguousImage => write!(f, "docker image load reported more than one image"),
            Self::InvalidReference(error) => {
                write!(f, "docker image reference is not immutable: {error}")
            }
            Self::StagedReference(error) => {
                write!(f, "staged image reference could not be written: {error}")
            }
        }
    }
}

impl Error for DockerImageImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ArtifactRead(error)
            | Self::WorkFile(error)
            | Self::ArchiveWrite(error)
            | Self::Spawn(error)
            | Self::Read(error)
            | Self::Wait(error)
            | Self::Terminate(error) => Some(error),
            Self::InvalidReference(error) => Some(error),
            Self::StagedReference(error) => Some(error),
            Self::ArtifactSymlink
            | Self::ArtifactNotRegular
            | Self::ArtifactTooLarge
            | Self::ArchiveEmpty
            | Self::ArchiveTooLarge
            | Self::ReaderPanicked
            | Self::TimedOut(_)
            | Self::OutputTooLarge(_)
            | Self::LoadFailed(_)
            | Self::InspectFailed(_)
            | Self::UnparsableOutput
            | Self::AmbiguousImage => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use rumahl_core::PackagePath;

    use super::*;
    use crate::test_support::{process_spawn_guard, unique_test_root};

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn write_script(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn fixture() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = unique_test_root('p');
        let package = root.join("package");
        fs::create_dir(&package).unwrap();
        fs::create_dir(package.join("runtime")).unwrap();
        fs::write(package.join("runtime/server.oci"), b"archive").unwrap();
        let work = root.join("work");
        fs::create_dir(&work).unwrap();
        let images = root.join("images");
        fs::create_dir(&images).unwrap();
        (root, package, work, images)
    }

    fn importer(executable: &Path, work: &Path, images: &Path) -> DockerImageImporter {
        DockerImageImporter::new(
            DockerImageImporterConfig::new(executable, work, images, Duration::from_secs(30))
                .unwrap(),
        )
    }

    fn reference_file(images: &Path, installation_id: &InstallationId) -> String {
        fs::read_to_string(
            images
                .join(installation_id.to_string())
                .join("image-reference"),
        )
        .unwrap()
    }

    #[test]
    fn imports_image_from_load_output() {
        let _guard = process_spawn_guard();
        let (root, package, work, images) = fixture();
        let executable = root.join("docker");
        write_script(
            &executable,
            &format!(
                "#!/bin/sh\ncase \"$1\" in\n  load) printf 'Loaded image ID: sha256:%s\\n' '{DIGEST_A}' >&2; exit 0 ;;\n  *) exit 1 ;;\nesac\n"
            ),
        );
        let installation_id = InstallationId::new();

        importer(&executable, &work, &images)
            .stage_image(
                &installation_id,
                &package,
                &PackagePath::parse("runtime/server.oci").unwrap(),
            )
            .unwrap();

        assert_eq!(
            reference_file(&images, &installation_id),
            format!("RDI1\nruntime/server.oci\nsha256:{DIGEST_A}\n")
        );
        assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspects_named_image_after_load() {
        let _guard = process_spawn_guard();
        let (root, package, work, images) = fixture();
        let executable = root.join("docker");
        write_script(
            &executable,
            &format!(
                "#!/bin/sh\ncase \"$1\" in\n  load) printf 'Loaded image: test/app:1\\n' >&2; exit 0 ;;\n  image) printf 'sha256:%s\\n' '{DIGEST_B}'; exit 0 ;;\n  *) exit 1 ;;\nesac\n"
            ),
        );
        let installation_id = InstallationId::new();

        importer(&executable, &work, &images)
            .stage_image(
                &installation_id,
                &package,
                &PackagePath::parse("runtime/server.oci").unwrap(),
            )
            .unwrap();

        assert_eq!(
            reference_file(&images, &installation_id),
            format!("RDI1\nruntime/server.oci\nsha256:{DIGEST_B}\n")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_failed_load() {
        let _guard = process_spawn_guard();
        let (root, package, work, images) = fixture();
        let executable = root.join("docker");
        write_script(&executable, "#!/bin/sh\nexit 2\n");

        let error = importer(&executable, &work, &images)
            .stage_image(
                &InstallationId::new(),
                &package,
                &PackagePath::parse("runtime/server.oci").unwrap(),
            )
            .unwrap_err();

        assert!(matches!(error, DockerImageImportError::LoadFailed(Some(2))));
        assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_symlinked_artifact() {
        let (root, package, work, images) = fixture();
        let artifact = package.join("runtime/server.oci");
        let outside = package.join("runtime/outside.oci");
        fs::rename(&artifact, &outside).unwrap();
        symlink(&outside, &artifact).unwrap();

        let error = importer(Path::new("/usr/bin/docker"), &work, &images)
            .stage_image(
                &InstallationId::new(),
                &package,
                &PackagePath::parse("runtime/server.oci").unwrap(),
            )
            .unwrap_err();

        assert!(matches!(error, DockerImageImportError::ArtifactSymlink));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_empty_archive() {
        let (root, _package, work, images) = fixture();
        let error = importer(Path::new("/usr/bin/docker"), &work, &images)
            .import_archive(
                &InstallationId::new(),
                &PackagePath::parse("runtime/server.oci").unwrap(),
                b"",
            )
            .unwrap_err();
        assert!(matches!(error, DockerImageImportError::ArchiveEmpty));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_configuration() {
        let root = unique_test_root('w');
        let work = root.join("work");
        let images = root.join("images");
        assert_eq!(
            DockerImageImporterConfig::new("docker", &work, &images, Duration::from_secs(30))
                .unwrap_err(),
            DockerImageImporterConfigError::ExecutableMustBeAbsolute
        );
        assert_eq!(
            DockerImageImporterConfig::new(
                "/usr/bin/docker",
                "work",
                &images,
                Duration::from_secs(30)
            )
            .unwrap_err(),
            DockerImageImporterConfigError::WorkRootMustBeAbsolute
        );
        assert_eq!(
            DockerImageImporterConfig::new(
                "/usr/bin/docker",
                &work,
                "images",
                Duration::from_secs(30)
            )
            .unwrap_err(),
            DockerImageImporterConfigError::ImageRootMustBeAbsolute
        );
        assert_eq!(
            DockerImageImporterConfig::new("/usr/bin/docker", &work, &images, Duration::ZERO)
                .unwrap_err(),
            DockerImageImporterConfigError::ZeroTimeout
        );
        fs::remove_dir_all(root).unwrap();
    }
}
