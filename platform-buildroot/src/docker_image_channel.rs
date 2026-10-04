use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumahl_core::PackagePath;

use crate::docker_image_importer::MAX_ARCHIVE_BYTES;
use crate::runtime_secrets::peer_uid;
use crate::{
    ContainerImageImporter, DockerImageReference, DockerImageReferenceError, DockerImageTarget,
};

pub(crate) const PROTOCOL_MAGIC: &[u8; 4] = b"DPI1";
pub(crate) const OP_IMPORT: u8 = 1;
pub(crate) const STATUS_OK: u8 = 0;
pub(crate) const STATUS_REJECTED: u8 = 2;
const HEADER_LENGTH: usize = 13;
const DIGEST_LENGTH: usize = 64;
const RESPONSE_OK_LENGTH: usize = 5 + DIGEST_LENGTH;
const OWNER_ONLY_SOCKET_MODE: u32 = 0o600;
const OWNER_GROUP_SOCKET_MODE: u32 = 0o660;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct UnixDockerImageImporterConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixDockerImageImporterConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
}

#[derive(Debug)]
pub struct UnixDockerImageImporter {
    config: UnixDockerImageImporterConfig,
}

#[derive(Debug)]
pub enum UnixDockerImageImporterError {
    ArtifactSymlink,
    ArtifactNotRegular,
    ArtifactTooLarge,
    ArtifactRead(io::Error),
    FieldTooLarge,
    Connect(io::Error),
    PeerCredentials(io::Error),
    UnexpectedPeerUid { expected: u32, actual: u32 },
    Write(io::Error),
    Read(io::Error),
    InvalidResponse,
    Rejected,
    InvalidReference(DockerImageReferenceError),
}

#[derive(Debug, Clone)]
pub struct UnixDockerImageServerConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
    socket_mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixDockerImageServerConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
    InvalidSocketMode,
}

pub struct UnixDockerImageServer<T> {
    config: UnixDockerImageServerConfig,
    listener: UnixListener,
    target: T,
}

#[derive(Debug)]
pub enum UnixDockerImageServerError {
    Bind(io::Error),
    SetSocketPermissions(io::Error),
    Accept(io::Error),
    ConfigureConnection(io::Error),
    PeerCredentials(io::Error),
    UnexpectedPeerUid { expected: u32, actual: u32 },
    Read(io::Error),
    InvalidRequest,
    Target,
    Write(io::Error),
}

impl UnixDockerImageImporterConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixDockerImageImporterConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixDockerImageImporterConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixDockerImageImporterConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixDockerImageImporterConfigError::ZeroTimeout);
        }
        Ok(Self {
            socket_path,
            expected_peer_uid,
            timeout,
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn expected_peer_uid(&self) -> u32 {
        self.expected_peer_uid
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl UnixDockerImageServerConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixDockerImageServerConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixDockerImageServerConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixDockerImageServerConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixDockerImageServerConfigError::ZeroTimeout);
        }
        Ok(Self {
            socket_path,
            expected_peer_uid,
            timeout,
            socket_mode: OWNER_ONLY_SOCKET_MODE,
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn expected_peer_uid(&self) -> u32 {
        self.expected_peer_uid
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn socket_mode(&self) -> u32 {
        self.socket_mode
    }

    pub fn with_socket_mode(
        mut self,
        socket_mode: u32,
    ) -> Result<Self, UnixDockerImageServerConfigError> {
        if !matches!(
            socket_mode,
            OWNER_ONLY_SOCKET_MODE | OWNER_GROUP_SOCKET_MODE
        ) {
            return Err(UnixDockerImageServerConfigError::InvalidSocketMode);
        }
        self.socket_mode = socket_mode;
        Ok(self)
    }
}

impl UnixDockerImageImporter {
    pub fn new(config: UnixDockerImageImporterConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &UnixDockerImageImporterConfig {
        &self.config
    }
}

impl ContainerImageImporter for UnixDockerImageImporter {
    type Error = UnixDockerImageImporterError;

    fn import_image(
        &self,
        package_root: &Path,
        artifact: &PackagePath,
    ) -> Result<DockerImageReference, Self::Error> {
        let path = package_root.join(artifact.as_str());
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| UnixDockerImageImporterError::ArtifactNotRegular)?;
        if metadata.file_type().is_symlink() {
            return Err(UnixDockerImageImporterError::ArtifactSymlink);
        }
        if !metadata.file_type().is_file() {
            return Err(UnixDockerImageImporterError::ArtifactNotRegular);
        }
        if metadata.len() > MAX_ARCHIVE_BYTES as u64 {
            return Err(UnixDockerImageImporterError::ArtifactTooLarge);
        }

        let archive = fs::read(&path).map_err(UnixDockerImageImporterError::ArtifactRead)?;
        self.exchange(&archive)
    }
}

impl UnixDockerImageImporter {
    fn exchange(
        &self,
        archive: &[u8],
    ) -> Result<DockerImageReference, UnixDockerImageImporterError> {
        if archive.is_empty() || archive.len() > MAX_ARCHIVE_BYTES {
            return Err(UnixDockerImageImporterError::FieldTooLarge);
        }

        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(UnixDockerImageImporterError::Connect)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixDockerImageImporterError::Connect)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixDockerImageImporterError::Connect)?;

        let actual_uid =
            peer_uid(&stream).map_err(UnixDockerImageImporterError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixDockerImageImporterError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        let mut header = [0_u8; HEADER_LENGTH];
        header[..4].copy_from_slice(PROTOCOL_MAGIC);
        header[4] = OP_IMPORT;
        header[5..].copy_from_slice(&(archive.len() as u64).to_be_bytes());
        stream
            .write_all(&header)
            .map_err(UnixDockerImageImporterError::Write)?;
        stream
            .write_all(archive)
            .map_err(UnixDockerImageImporterError::Write)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(UnixDockerImageImporterError::Write)?;

        let mut response = [0_u8; RESPONSE_OK_LENGTH];
        stream
            .read_exact(&mut response[..5])
            .map_err(UnixDockerImageImporterError::Read)?;
        if &response[..4] != PROTOCOL_MAGIC {
            return Err(UnixDockerImageImporterError::InvalidResponse);
        }

        match response[4] {
            STATUS_OK => {
                stream
                    .read_exact(&mut response[5..])
                    .map_err(UnixDockerImageImporterError::Read)?;
                let digest = std::str::from_utf8(&response[5..])
                    .map_err(|_| UnixDockerImageImporterError::InvalidResponse)?;
                DockerImageReference::parse(format!("sha256:{digest}"))
                    .map_err(UnixDockerImageImporterError::InvalidReference)
            }
            STATUS_REJECTED => Err(UnixDockerImageImporterError::Rejected),
            _ => Err(UnixDockerImageImporterError::InvalidResponse),
        }
    }
}

impl<T> UnixDockerImageServer<T>
where
    T: DockerImageTarget,
{
    pub fn bind(
        config: UnixDockerImageServerConfig,
        target: T,
    ) -> Result<Self, UnixDockerImageServerError> {
        let listener =
            UnixListener::bind(&config.socket_path).map_err(UnixDockerImageServerError::Bind)?;
        if let Err(error) = fs::set_permissions(
            &config.socket_path,
            fs::Permissions::from_mode(config.socket_mode),
        ) {
            drop(listener);
            let _ = fs::remove_file(&config.socket_path);
            return Err(UnixDockerImageServerError::SetSocketPermissions(error));
        }
        Ok(Self {
            config,
            listener,
            target,
        })
    }

    pub fn config(&self) -> &UnixDockerImageServerConfig {
        &self.config
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn serve_once(&self) -> Result<(), UnixDockerImageServerError> {
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(UnixDockerImageServerError::Accept)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixDockerImageServerError::ConfigureConnection)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixDockerImageServerError::ConfigureConnection)?;

        let actual_uid = peer_uid(&stream).map_err(UnixDockerImageServerError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixDockerImageServerError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        let archive = match read_request(&mut stream) {
            Ok(archive) => archive,
            Err(error) => {
                write_status(&mut stream, STATUS_REJECTED)
                    .map_err(UnixDockerImageServerError::Write)?;
                return Err(error);
            }
        };

        match self.target.import_archive(&archive) {
            Ok(reference) => write_reference(&mut stream, reference.as_str())
                .map_err(UnixDockerImageServerError::Write),
            Err(_) => {
                write_status(&mut stream, STATUS_REJECTED)
                    .map_err(UnixDockerImageServerError::Write)?;
                Err(UnixDockerImageServerError::Target)
            }
        }
    }
}

fn read_request(stream: &mut UnixStream) -> Result<Vec<u8>, UnixDockerImageServerError> {
    let mut header = [0_u8; HEADER_LENGTH];
    stream
        .read_exact(&mut header)
        .map_err(UnixDockerImageServerError::Read)?;
    if &header[..4] != PROTOCOL_MAGIC || header[4] != OP_IMPORT {
        return Err(UnixDockerImageServerError::InvalidRequest);
    }

    let length = u64::from_be_bytes(header[5..].try_into().expect("header has eight bytes"));
    let length = usize::try_from(length).map_err(|_| UnixDockerImageServerError::InvalidRequest)?;
    if length == 0 || length > MAX_ARCHIVE_BYTES {
        return Err(UnixDockerImageServerError::InvalidRequest);
    }

    let mut archive = vec![0_u8; length];
    stream
        .read_exact(&mut archive)
        .map_err(UnixDockerImageServerError::Read)?;

    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing) {
        Ok(0) => Ok(archive),
        Ok(_) => Err(UnixDockerImageServerError::InvalidRequest),
        Err(error) => Err(UnixDockerImageServerError::Read(error)),
    }
}

fn write_status(stream: &mut UnixStream, status: u8) -> io::Result<()> {
    stream.write_all(&[
        PROTOCOL_MAGIC[0],
        PROTOCOL_MAGIC[1],
        PROTOCOL_MAGIC[2],
        PROTOCOL_MAGIC[3],
        status,
    ])
}

fn write_reference(stream: &mut UnixStream, reference: &str) -> io::Result<()> {
    let digest = reference
        .rsplit("sha256:")
        .next()
        .filter(|digest| digest.len() == DIGEST_LENGTH)
        .unwrap_or("");
    stream.write_all(PROTOCOL_MAGIC)?;
    stream.write_all(&[STATUS_OK])?;
    stream.write_all(digest.as_bytes())
}

impl fmt::Display for UnixDockerImageImporterConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "docker image import socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "docker image import timeout must be non-zero"),
        }
    }
}

impl Error for UnixDockerImageImporterConfigError {}

impl fmt::Display for UnixDockerImageImporterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactSymlink => write!(f, "package image artifact must not be a symlink"),
            Self::ArtifactNotRegular => {
                write!(f, "package image artifact is not a regular file")
            }
            Self::ArtifactTooLarge => write!(f, "package image artifact exceeds the size limit"),
            Self::ArtifactRead(_) => write!(f, "package image artifact could not be read"),
            Self::FieldTooLarge => write!(f, "docker image import archive is empty or too large"),
            Self::Connect(_) => write!(f, "docker image import socket connection failed"),
            Self::PeerCredentials(_) => {
                write!(f, "docker image import socket peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "docker image import socket peer UID {actual} does not match expected UID {expected}"
            ),
            Self::Write(_) => write!(f, "docker image import request failed"),
            Self::Read(_) => write!(f, "docker image import response failed"),
            Self::InvalidResponse => write!(f, "docker image import response is invalid"),
            Self::Rejected => write!(f, "docker image import was rejected"),
            Self::InvalidReference(error) => {
                write!(f, "docker image reference is not immutable: {error}")
            }
        }
    }
}

impl Error for UnixDockerImageImporterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ArtifactRead(error)
            | Self::Connect(error)
            | Self::PeerCredentials(error)
            | Self::Write(error)
            | Self::Read(error) => Some(error),
            Self::InvalidReference(error) => Some(error),
            Self::ArtifactSymlink
            | Self::ArtifactNotRegular
            | Self::ArtifactTooLarge
            | Self::FieldTooLarge
            | Self::UnexpectedPeerUid { .. }
            | Self::InvalidResponse
            | Self::Rejected => None,
        }
    }
}

impl fmt::Display for UnixDockerImageServerConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "docker image server socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "docker image server timeout must be non-zero"),
            Self::InvalidSocketMode => {
                write!(f, "docker image server socket mode must be 0600 or 0660")
            }
        }
    }
}

impl Error for UnixDockerImageServerConfigError {}

impl fmt::Display for UnixDockerImageServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(_) => write!(f, "docker image server socket bind failed"),
            Self::SetSocketPermissions(_) => {
                write!(f, "docker image server socket permission setup failed")
            }
            Self::Accept(_) => write!(f, "docker image server accept failed"),
            Self::ConfigureConnection(_) => {
                write!(f, "docker image server connection setup failed")
            }
            Self::PeerCredentials(_) => {
                write!(f, "docker image server peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "docker image client UID {actual} does not match expected UID {expected}"
            ),
            Self::Read(_) => write!(f, "docker image request read failed"),
            Self::InvalidRequest => write!(f, "docker image request is invalid"),
            Self::Target => write!(f, "docker image target rejected the import"),
            Self::Write(_) => write!(f, "docker image response write failed"),
        }
    }
}

impl Error for UnixDockerImageServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bind(error)
            | Self::SetSocketPermissions(error)
            | Self::Accept(error)
            | Self::ConfigureConnection(error)
            | Self::PeerCredentials(error)
            | Self::Read(error)
            | Self::Write(error) => Some(error),
            Self::UnexpectedPeerUid { .. } | Self::InvalidRequest | Self::Target => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::*;
    use crate::test_support::unique_test_root;

    const DIGEST: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn current_uid() -> u32 {
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() }
    }

    fn reference(digest: &str) -> DockerImageReference {
        DockerImageReference::parse(format!("sha256:{digest}")).unwrap()
    }

    #[derive(Clone, Default)]
    struct RecordingTarget {
        archives: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl DockerImageTarget for RecordingTarget {
        type Error = Infallible;

        fn import_archive(&self, archive: &[u8]) -> Result<DockerImageReference, Self::Error> {
            self.archives.lock().unwrap().push(archive.to_vec());
            Ok(reference(DIGEST))
        }
    }

    struct FailingTarget;

    impl DockerImageTarget for FailingTarget {
        type Error = io::Error;

        fn import_archive(&self, _archive: &[u8]) -> Result<DockerImageReference, Self::Error> {
            Err(io::Error::other("image import failed"))
        }
    }

    fn write_artifact(root: &Path, contents: &[u8]) -> (PathBuf, PackagePath) {
        let package = root.join("package");
        fs::create_dir_all(package.join("runtime")).unwrap();
        fs::write(package.join("runtime/server.oci"), contents).unwrap();
        (package, PackagePath::parse("runtime/server.oci").unwrap())
    }

    #[test]
    fn imports_archive_over_authenticated_socket() {
        let root = unique_test_root('q');
        let socket_path = root.join("image.sock");
        let target = RecordingTarget::default();
        let server = UnixDockerImageServer::bind(
            UnixDockerImageServerConfig::new(&socket_path, current_uid()).unwrap(),
            target.clone(),
        )
        .unwrap();
        let (package, artifact) = write_artifact(&root, b"archive-bytes");

        let worker = thread::spawn(move || server.serve_once());
        let importer = UnixDockerImageImporter::new(
            UnixDockerImageImporterConfig::new(&socket_path, current_uid()).unwrap(),
        );
        let image = importer.import_image(&package, &artifact).unwrap();
        worker.join().unwrap().unwrap();

        assert_eq!(image.as_str(), format!("sha256:{DIGEST}"));
        assert_eq!(
            target.archives.lock().unwrap().as_slice(),
            [b"archive-bytes".to_vec()]
        );
        assert_eq!(
            fs::metadata(&socket_path).unwrap().permissions().mode() & 0o777,
            OWNER_ONLY_SOCKET_MODE
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn maps_rejected_target_to_client_failure() {
        let root = unique_test_root('q');
        let socket_path = root.join("image.sock");
        let server = UnixDockerImageServer::bind(
            UnixDockerImageServerConfig::new(&socket_path, current_uid()).unwrap(),
            FailingTarget,
        )
        .unwrap();
        let (package, artifact) = write_artifact(&root, b"archive-bytes");

        let worker = thread::spawn(move || server.serve_once());
        let importer = UnixDockerImageImporter::new(
            UnixDockerImageImporterConfig::new(&socket_path, current_uid()).unwrap(),
        );
        let error = importer.import_image(&package, &artifact).unwrap_err();

        assert!(matches!(error, UnixDockerImageImporterError::Rejected));
        assert!(matches!(
            worker.join().unwrap(),
            Err(UnixDockerImageServerError::Target)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_oversized_request_header() {
        let root = unique_test_root('q');
        let socket_path = root.join("image.sock");
        let target = RecordingTarget::default();
        let server = UnixDockerImageServer::bind(
            UnixDockerImageServerConfig::new(&socket_path, current_uid()).unwrap(),
            target.clone(),
        )
        .unwrap();

        let worker = thread::spawn(move || server.serve_once());
        let mut stream = UnixStream::connect(&socket_path).unwrap();
        let mut header = Vec::from(*PROTOCOL_MAGIC);
        header.push(OP_IMPORT);
        header.extend_from_slice(&(u64::MAX).to_be_bytes());
        stream.write_all(&header).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = [0_u8; 5];
        stream.read_exact(&mut response).unwrap();

        assert_eq!(response[4], STATUS_REJECTED);
        assert!(matches!(
            worker.join().unwrap(),
            Err(UnixDockerImageServerError::InvalidRequest)
        ));
        assert!(target.archives.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_configuration() {
        assert_eq!(
            UnixDockerImageImporterConfig::new("relative.sock", 1000).unwrap_err(),
            UnixDockerImageImporterConfigError::SocketPathMustBeAbsolute
        );
        assert_eq!(
            UnixDockerImageServerConfig::new("/run/rumahl/image.sock", 1000)
                .unwrap()
                .with_socket_mode(0o666)
                .unwrap_err(),
            UnixDockerImageServerConfigError::InvalidSocketMode
        );
    }
}
