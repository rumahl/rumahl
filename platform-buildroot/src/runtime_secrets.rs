use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumahl_app_operations::RuntimeSecretDelivery;
use rumahl_core::{AppOperationId, InstallationId, InstalledApp};
use rumahl_oidc_provider::{OidcClientId, OidcClientSecret};
use zeroize::Zeroizing;

const PROTOCOL_MAGIC: &[u8; 4] = b"RSH1";
const OP_DELIVER_OIDC: u8 = 1;
const OP_REMOVE_INSTALLATION: u8 = 2;
const STATUS_OK: u8 = 0;
const STATUS_CONFLICT: u8 = 1;
const STATUS_REJECTED: u8 = 2;
const RESPONSE_LENGTH: usize = 5;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct UnixRuntimeSecretDeliveryConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixRuntimeSecretDeliveryConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
}

#[derive(Debug, Clone)]
pub struct UnixRuntimeSecretDelivery {
    config: UnixRuntimeSecretDeliveryConfig,
}

#[derive(Debug)]
pub enum UnixRuntimeSecretDeliveryError {
    Connect(io::Error),
    PeerCredentials(io::Error),
    UnexpectedPeerUid { expected: u32, actual: u32 },
    FieldTooLarge,
    Write(io::Error),
    Read(io::Error),
    InvalidResponse,
    ConflictingReplay,
    Rejected,
}

impl UnixRuntimeSecretDeliveryConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixRuntimeSecretDeliveryConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixRuntimeSecretDeliveryConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixRuntimeSecretDeliveryConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixRuntimeSecretDeliveryConfigError::ZeroTimeout);
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

impl UnixRuntimeSecretDelivery {
    pub fn new(config: UnixRuntimeSecretDeliveryConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &UnixRuntimeSecretDeliveryConfig {
        &self.config
    }

    fn exchange(&self, request: &[u8]) -> Result<(), UnixRuntimeSecretDeliveryError> {
        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(UnixRuntimeSecretDeliveryError::Connect)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeSecretDeliveryError::Connect)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeSecretDeliveryError::Connect)?;

        let actual_uid =
            peer_uid(&stream).map_err(UnixRuntimeSecretDeliveryError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixRuntimeSecretDeliveryError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        stream
            .write_all(request)
            .map_err(UnixRuntimeSecretDeliveryError::Write)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(UnixRuntimeSecretDeliveryError::Write)?;

        let mut response = [0_u8; RESPONSE_LENGTH];
        stream
            .read_exact(&mut response)
            .map_err(UnixRuntimeSecretDeliveryError::Read)?;
        if &response[..PROTOCOL_MAGIC.len()] != PROTOCOL_MAGIC {
            return Err(UnixRuntimeSecretDeliveryError::InvalidResponse);
        }

        match response[PROTOCOL_MAGIC.len()] {
            STATUS_OK => Ok(()),
            STATUS_CONFLICT => Err(UnixRuntimeSecretDeliveryError::ConflictingReplay),
            STATUS_REJECTED => Err(UnixRuntimeSecretDeliveryError::Rejected),
            _ => Err(UnixRuntimeSecretDeliveryError::InvalidResponse),
        }
    }
}

impl RuntimeSecretDelivery for UnixRuntimeSecretDelivery {
    type Error = UnixRuntimeSecretDeliveryError;

    fn deliver_oidc_client_secret(
        &self,
        operation_id: &AppOperationId,
        app: &InstalledApp,
        client_id: &OidcClientId,
        client_secret: &OidcClientSecret,
    ) -> Result<(), Self::Error> {
        let operation_id = operation_id.to_string();
        let installation_id = app.installation_id().to_string();
        let encoded_secret = client_secret.encode();
        let fields: [&[u8]; 6] = [
            operation_id.as_bytes(),
            installation_id.as_bytes(),
            app.identity().app_id().as_str().as_bytes(),
            app.identity().publisher_id().as_str().as_bytes(),
            client_id.as_str().as_bytes(),
            encoded_secret.as_bytes(),
        ];
        let request = Zeroizing::new(encode_request(OP_DELIVER_OIDC, &fields)?);
        self.exchange(&request)
    }

    fn remove_for_installation(
        &self,
        operation_id: &AppOperationId,
        installation_id: &InstallationId,
    ) -> Result<(), Self::Error> {
        let operation_id = operation_id.to_string();
        let installation_id = installation_id.to_string();
        let fields = [operation_id.as_bytes(), installation_id.as_bytes()];
        let request = encode_request(OP_REMOVE_INSTALLATION, &fields)?;
        self.exchange(&request)
    }
}

fn encode_request(
    operation: u8,
    fields: &[&[u8]],
) -> Result<Vec<u8>, UnixRuntimeSecretDeliveryError> {
    let field_count =
        u8::try_from(fields.len()).map_err(|_| UnixRuntimeSecretDeliveryError::FieldTooLarge)?;
    let mut request = Vec::new();
    request.extend_from_slice(PROTOCOL_MAGIC);
    request.push(operation);
    request.push(field_count);

    for field in fields {
        let length = u32::try_from(field.len())
            .map_err(|_| UnixRuntimeSecretDeliveryError::FieldTooLarge)?;
        request.extend_from_slice(&length.to_be_bytes());
        request.extend_from_slice(field);
    }

    Ok(request)
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `credentials` and `length` point to writable objects of the
    // exact sizes supplied to `getsockopt`, and the stream owns a valid fd.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast::<libc::c_void>(),
            &mut length,
        )
    };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected peer credential length",
        ));
    }
    Ok(credentials.uid)
}

#[cfg(target_os = "macos")]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut uid = 0;
    let mut gid = 0;
    // SAFETY: both output pointers are valid and the stream owns a valid fd.
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

impl fmt::Display for UnixRuntimeSecretDeliveryConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "runtime secret socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "runtime secret socket timeout must be non-zero"),
        }
    }
}

impl Error for UnixRuntimeSecretDeliveryConfigError {}

impl fmt::Display for UnixRuntimeSecretDeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(_) => write!(f, "runtime secret socket connection failed"),
            Self::PeerCredentials(_) => {
                write!(f, "runtime secret socket peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "runtime secret socket peer UID {actual} does not match expected UID {expected}"
            ),
            Self::FieldTooLarge => write!(f, "runtime secret protocol field is too large"),
            Self::Write(_) => write!(f, "runtime secret request failed"),
            Self::Read(_) => write!(f, "runtime secret acknowledgement failed"),
            Self::InvalidResponse => write!(f, "runtime secret acknowledgement is invalid"),
            Self::ConflictingReplay => {
                write!(
                    f,
                    "runtime secret delivery conflicts with an earlier replay"
                )
            }
            Self::Rejected => write!(f, "runtime secret delivery was rejected"),
        }
    }
}

impl Error for UnixRuntimeSecretDeliveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error)
            | Self::PeerCredentials(error)
            | Self::Write(error)
            | Self::Read(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_core::{
        AppId, AppManifest, AppManifestValidator, AppVersion, PackagePath, PublisherId,
        RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    use super::*;

    fn test_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        PathBuf::from("/private/tmp").join(format!("rmh-rs-{}-{nonce}", std::process::id()))
    }

    fn current_uid() -> u32 {
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() }
    }

    fn installed_app() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    fn receive_one(socket_path: PathBuf, response_status: u8) -> thread::JoinHandle<Vec<u8>> {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            stream
                .write_all(&[
                    PROTOCOL_MAGIC[0],
                    PROTOCOL_MAGIC[1],
                    PROTOCOL_MAGIC[2],
                    PROTOCOL_MAGIC[3],
                    response_status,
                ])
                .unwrap();
            request
        })
    }

    fn decode_fields(request: &[u8]) -> Vec<&[u8]> {
        let mut position = 6;
        let mut fields = Vec::new();
        for _ in 0..request[5] {
            let length =
                u32::from_be_bytes(request[position..position + 4].try_into().unwrap()) as usize;
            position += 4;
            fields.push(&request[position..position + length]);
            position += length;
        }
        assert_eq!(position, request.len());
        fields
    }

    #[test]
    fn sends_oidc_secret_only_inside_authenticated_socket_frame() {
        let root = test_root();
        fs::create_dir(&root).unwrap();
        let socket_path = root.join("runtime.sock");
        let server = receive_one(socket_path.clone(), STATUS_OK);
        let delivery = UnixRuntimeSecretDelivery::new(
            UnixRuntimeSecretDeliveryConfig::new(socket_path, current_uid()).unwrap(),
        );
        let operation_id = AppOperationId::new();
        let app = installed_app();
        let client_id = OidcClientId::generate().unwrap();
        let secret = OidcClientSecret::generate().unwrap();
        let encoded_secret = secret.encode();

        delivery
            .deliver_oidc_client_secret(&operation_id, &app, &client_id, &secret)
            .unwrap();

        let request = server.join().unwrap();
        assert_eq!(&request[..4], PROTOCOL_MAGIC);
        assert_eq!(request[4], OP_DELIVER_OIDC);
        assert_eq!(request[5], 6);
        let fields = decode_fields(&request);
        assert_eq!(fields[0], operation_id.to_string().as_bytes());
        assert_eq!(fields[1], app.installation_id().to_string().as_bytes());
        assert_eq!(fields[2], b"com.rumahl.notes");
        assert_eq!(fields[3], b"com.rumahl");
        assert_eq!(fields[4], client_id.as_str().as_bytes());
        assert_eq!(fields[5], encoded_secret.as_bytes());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn maps_supervisor_replay_conflict_to_closed_failure() {
        let root = test_root();
        fs::create_dir(&root).unwrap();
        let socket_path = root.join("runtime.sock");
        let server = receive_one(socket_path.clone(), STATUS_CONFLICT);
        let delivery = UnixRuntimeSecretDelivery::new(
            UnixRuntimeSecretDeliveryConfig::new(socket_path, current_uid()).unwrap(),
        );

        let error = delivery
            .remove_for_installation(&AppOperationId::new(), &InstallationId::new())
            .unwrap_err();

        assert!(matches!(
            error,
            UnixRuntimeSecretDeliveryError::ConflictingReplay
        ));
        let request = server.join().unwrap();
        assert_eq!(request[4], OP_REMOVE_INSTALLATION);
        assert_eq!(request[5], 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unexpected_socket_peer_uid_before_sending_secret() {
        let root = test_root();
        fs::create_dir(&root).unwrap();
        let socket_path = root.join("runtime.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            request
        });
        let unexpected_uid = current_uid().wrapping_add(1);
        let delivery = UnixRuntimeSecretDelivery::new(
            UnixRuntimeSecretDeliveryConfig::new(socket_path, unexpected_uid).unwrap(),
        );

        let error = delivery
            .remove_for_installation(&AppOperationId::new(), &InstallationId::new())
            .unwrap_err();

        assert!(matches!(
            error,
            UnixRuntimeSecretDeliveryError::UnexpectedPeerUid { .. }
        ));
        assert!(server.join().unwrap().is_empty());

        fs::remove_dir_all(root).unwrap();
    }
}
