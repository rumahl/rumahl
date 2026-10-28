use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumahl_core::{AppId, AppOperationId, InstallationId, PublisherId};
use rumahl_oidc_provider::{OidcClientId, OidcClientSecret};
use zeroize::Zeroizing;

use crate::runtime_secrets::{
    OP_DELIVER_OIDC, OP_REMOVE_INSTALLATION, PROTOCOL_MAGIC, STATUS_CONFLICT, STATUS_OK,
    STATUS_REJECTED, peer_uid,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const OWNER_ONLY_SOCKET_MODE: u32 = 0o600;
const OWNER_GROUP_SOCKET_MODE: u32 = 0o660;
const MAX_FIELD_LENGTH: usize = 1024;
const MAX_REQUEST_LENGTH: usize = 4096;
const DELIVER_FIELD_COUNT: u8 = 6;
const REMOVE_FIELD_COUNT: u8 = 2;

#[derive(Debug, Clone)]
pub struct UnixRuntimeSecretServerConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
    socket_mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixRuntimeSecretServerConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
    InvalidSocketMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSecretTargetOutcome {
    Applied,
    AlreadyApplied,
    Conflict,
    Rejected,
}

#[derive(Debug)]
pub struct RuntimeOidcClientSecret {
    operation_id: AppOperationId,
    installation_id: InstallationId,
    app_id: AppId,
    publisher_id: PublisherId,
    client_id: OidcClientId,
    client_secret: OidcClientSecret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSecretRemoval {
    operation_id: AppOperationId,
    installation_id: InstallationId,
}

/// Privileged supervisor boundary for volatile runtime material.
///
/// Implementations must apply a delivery only to the named installation's
/// runtime namespace. They must treat an identical replay as
/// [`RuntimeSecretTargetOutcome::AlreadyApplied`] and a replay with different
/// material for the same operation and installation as
/// [`RuntimeSecretTargetOutcome::Conflict`]. Plaintext must not be written to
/// persistent storage, logs, process arguments, or environment variables.
pub trait RuntimeSecretTarget {
    type Error: Error + Send + Sync + 'static;

    fn deliver_oidc_client_secret(
        &self,
        request: RuntimeOidcClientSecret,
    ) -> Result<RuntimeSecretTargetOutcome, Self::Error>;

    fn remove_for_installation(
        &self,
        request: RuntimeSecretRemoval,
    ) -> Result<RuntimeSecretTargetOutcome, Self::Error>;
}

pub struct UnixRuntimeSecretServer<T> {
    config: UnixRuntimeSecretServerConfig,
    listener: UnixListener,
    target: T,
}

#[derive(Debug)]
pub enum UnixRuntimeSecretServerError {
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

enum ParsedRequest {
    Deliver(RuntimeOidcClientSecret),
    Remove(RuntimeSecretRemoval),
}

impl UnixRuntimeSecretServerConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixRuntimeSecretServerConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixRuntimeSecretServerConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixRuntimeSecretServerConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixRuntimeSecretServerConfigError::ZeroTimeout);
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

    pub fn with_socket_mode(
        mut self,
        socket_mode: u32,
    ) -> Result<Self, UnixRuntimeSecretServerConfigError> {
        if !matches!(
            socket_mode,
            OWNER_ONLY_SOCKET_MODE | OWNER_GROUP_SOCKET_MODE
        ) {
            return Err(UnixRuntimeSecretServerConfigError::InvalidSocketMode);
        }
        self.socket_mode = socket_mode;
        Ok(self)
    }

    pub fn socket_mode(&self) -> u32 {
        self.socket_mode
    }
}

impl RuntimeOidcClientSecret {
    pub fn new(
        operation_id: AppOperationId,
        installation_id: InstallationId,
        app_id: AppId,
        publisher_id: PublisherId,
        client_id: OidcClientId,
        client_secret: OidcClientSecret,
    ) -> Self {
        Self {
            operation_id,
            installation_id,
            app_id,
            publisher_id,
            client_id,
            client_secret,
        }
    }

    pub fn operation_id(&self) -> &AppOperationId {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn app_id(&self) -> &AppId {
        &self.app_id
    }

    pub fn publisher_id(&self) -> &PublisherId {
        &self.publisher_id
    }

    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }

    pub fn client_secret(&self) -> &OidcClientSecret {
        &self.client_secret
    }
}

impl RuntimeSecretRemoval {
    pub fn new(operation_id: AppOperationId, installation_id: InstallationId) -> Self {
        Self {
            operation_id,
            installation_id,
        }
    }

    pub fn operation_id(&self) -> &AppOperationId {
        &self.operation_id
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }
}

impl<T> UnixRuntimeSecretServer<T>
where
    T: RuntimeSecretTarget,
{
    /// Binds a new socket and restricts it to its owner.
    ///
    /// An existing path is never removed. The service manager should create a
    /// private volatile parent directory and clean stale sockets before the
    /// process starts.
    pub fn bind(
        config: UnixRuntimeSecretServerConfig,
        target: T,
    ) -> Result<Self, UnixRuntimeSecretServerError> {
        let listener =
            UnixListener::bind(&config.socket_path).map_err(UnixRuntimeSecretServerError::Bind)?;
        if let Err(error) = fs::set_permissions(
            &config.socket_path,
            fs::Permissions::from_mode(config.socket_mode),
        ) {
            drop(listener);
            let _ = fs::remove_file(&config.socket_path);
            return Err(UnixRuntimeSecretServerError::SetSocketPermissions(error));
        }

        Ok(Self {
            config,
            listener,
            target,
        })
    }

    pub fn config(&self) -> &UnixRuntimeSecretServerConfig {
        &self.config
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    /// Accepts and handles one connection.
    ///
    /// A supervisor can call this in its own bounded worker loop. Peer
    /// authentication occurs before any request bytes are read.
    pub fn serve_once(&self) -> Result<(), UnixRuntimeSecretServerError> {
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(UnixRuntimeSecretServerError::Accept)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeSecretServerError::ConfigureConnection)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeSecretServerError::ConfigureConnection)?;

        let actual_uid =
            peer_uid(&stream).map_err(UnixRuntimeSecretServerError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixRuntimeSecretServerError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        let request = match parse_request(&mut stream) {
            Ok(request) => request,
            Err(error) => {
                write_status(&mut stream, STATUS_REJECTED)
                    .map_err(UnixRuntimeSecretServerError::Write)?;
                return Err(error);
            }
        };

        let outcome = match request {
            ParsedRequest::Deliver(request) => self
                .target
                .deliver_oidc_client_secret(request)
                .map_err(|_| UnixRuntimeSecretServerError::Target),
            ParsedRequest::Remove(request) => self
                .target
                .remove_for_installation(request)
                .map_err(|_| UnixRuntimeSecretServerError::Target),
        };

        match outcome {
            Ok(
                RuntimeSecretTargetOutcome::Applied | RuntimeSecretTargetOutcome::AlreadyApplied,
            ) => write_status(&mut stream, STATUS_OK).map_err(UnixRuntimeSecretServerError::Write),
            Ok(RuntimeSecretTargetOutcome::Conflict) => write_status(&mut stream, STATUS_CONFLICT)
                .map_err(UnixRuntimeSecretServerError::Write),
            Ok(RuntimeSecretTargetOutcome::Rejected) => write_status(&mut stream, STATUS_REJECTED)
                .map_err(UnixRuntimeSecretServerError::Write),
            Err(error) => {
                write_status(&mut stream, STATUS_REJECTED)
                    .map_err(UnixRuntimeSecretServerError::Write)?;
                Err(error)
            }
        }
    }
}

fn parse_request(stream: &mut UnixStream) -> Result<ParsedRequest, UnixRuntimeSecretServerError> {
    let mut header = [0_u8; 6];
    stream
        .read_exact(&mut header)
        .map_err(UnixRuntimeSecretServerError::Read)?;
    if &header[..4] != PROTOCOL_MAGIC {
        return Err(UnixRuntimeSecretServerError::InvalidRequest);
    }

    let operation = header[4];
    let expected_fields = match operation {
        OP_DELIVER_OIDC => DELIVER_FIELD_COUNT,
        OP_REMOVE_INSTALLATION => REMOVE_FIELD_COUNT,
        _ => return Err(UnixRuntimeSecretServerError::InvalidRequest),
    };
    if header[5] != expected_fields {
        return Err(UnixRuntimeSecretServerError::InvalidRequest);
    }

    let mut total_length = header.len();
    let mut fields = Vec::with_capacity(usize::from(expected_fields));
    for _ in 0..expected_fields {
        let mut length_bytes = [0_u8; 4];
        stream
            .read_exact(&mut length_bytes)
            .map_err(UnixRuntimeSecretServerError::Read)?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
        total_length = total_length
            .checked_add(length_bytes.len())
            .and_then(|length_so_far| length_so_far.checked_add(length))
            .ok_or(UnixRuntimeSecretServerError::InvalidRequest)?;
        if length == 0 || length > MAX_FIELD_LENGTH || total_length > MAX_REQUEST_LENGTH {
            return Err(UnixRuntimeSecretServerError::InvalidRequest);
        }

        let mut field = Zeroizing::new(vec![0_u8; length]);
        stream
            .read_exact(&mut field)
            .map_err(UnixRuntimeSecretServerError::Read)?;
        fields.push(field);
    }

    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing) {
        Ok(0) => {}
        Ok(_) => return Err(UnixRuntimeSecretServerError::InvalidRequest),
        Err(error) => return Err(UnixRuntimeSecretServerError::Read(error)),
    }

    match operation {
        OP_DELIVER_OIDC => parse_delivery(&fields),
        OP_REMOVE_INSTALLATION => parse_removal(&fields),
        _ => unreachable!("operation was validated above"),
    }
}

fn parse_delivery(
    fields: &[Zeroizing<Vec<u8>>],
) -> Result<ParsedRequest, UnixRuntimeSecretServerError> {
    let operation_id = AppOperationId::parse(field_as_str(&fields[0])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let installation_id = InstallationId::parse(field_as_str(&fields[1])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let app_id = AppId::parse(field_as_str(&fields[2])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let publisher_id = PublisherId::parse(field_as_str(&fields[3])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let client_id = OidcClientId::parse(field_as_str(&fields[4])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let client_secret = OidcClientSecret::parse(field_as_str(&fields[5])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;

    Ok(ParsedRequest::Deliver(RuntimeOidcClientSecret {
        operation_id,
        installation_id,
        app_id,
        publisher_id,
        client_id,
        client_secret,
    }))
}

fn parse_removal(
    fields: &[Zeroizing<Vec<u8>>],
) -> Result<ParsedRequest, UnixRuntimeSecretServerError> {
    let operation_id = AppOperationId::parse(field_as_str(&fields[0])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;
    let installation_id = InstallationId::parse(field_as_str(&fields[1])?)
        .map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)?;

    Ok(ParsedRequest::Remove(RuntimeSecretRemoval {
        operation_id,
        installation_id,
    }))
}

fn field_as_str(field: &[u8]) -> Result<&str, UnixRuntimeSecretServerError> {
    std::str::from_utf8(field).map_err(|_| UnixRuntimeSecretServerError::InvalidRequest)
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

impl fmt::Display for UnixRuntimeSecretServerConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "runtime secret server socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "runtime secret server timeout must be non-zero"),
            Self::InvalidSocketMode => {
                write!(f, "runtime secret server socket mode must be 0600 or 0660")
            }
        }
    }
}

impl Error for UnixRuntimeSecretServerConfigError {}

impl fmt::Display for UnixRuntimeSecretServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(_) => write!(f, "runtime secret server socket bind failed"),
            Self::SetSocketPermissions(_) => {
                write!(f, "runtime secret server socket permission setup failed")
            }
            Self::Accept(_) => write!(f, "runtime secret server accept failed"),
            Self::ConfigureConnection(_) => {
                write!(f, "runtime secret server connection setup failed")
            }
            Self::PeerCredentials(_) => {
                write!(f, "runtime secret server peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "runtime secret client UID {actual} does not match expected UID {expected}"
            ),
            Self::Read(_) => write!(f, "runtime secret request read failed"),
            Self::InvalidRequest => write!(f, "runtime secret request is invalid"),
            Self::Target => write!(f, "runtime secret target rejected the operation"),
            Self::Write(_) => write!(f, "runtime secret response write failed"),
        }
    }
}

impl Error for UnixRuntimeSecretServerError {
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

    use rumahl_app_operations::RuntimeSecretDelivery;
    use rumahl_core::{
        AppManifest, AppManifestValidator, AppVersion, InstalledApp, PackagePath,
        RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    use super::*;
    use crate::test_support::unique_test_root;
    use crate::{UnixRuntimeSecretDelivery, UnixRuntimeSecretDeliveryConfig};

    type RecordedDelivery = (AppOperationId, InstallationId, String, String);

    #[derive(Clone, Default)]
    struct RecordingTarget {
        deliveries: Arc<Mutex<Vec<RecordedDelivery>>>,
        removals: Arc<Mutex<Vec<(AppOperationId, InstallationId)>>>,
        outcome: Arc<Mutex<Option<RuntimeSecretTargetOutcome>>>,
    }

    impl RuntimeSecretTarget for RecordingTarget {
        type Error = Infallible;

        fn deliver_oidc_client_secret(
            &self,
            request: RuntimeOidcClientSecret,
        ) -> Result<RuntimeSecretTargetOutcome, Self::Error> {
            self.deliveries.lock().unwrap().push((
                *request.operation_id(),
                *request.installation_id(),
                request.client_id().to_string(),
                request.client_secret().encode().to_string(),
            ));
            Ok(self
                .outcome
                .lock()
                .unwrap()
                .unwrap_or(RuntimeSecretTargetOutcome::Applied))
        }

        fn remove_for_installation(
            &self,
            request: RuntimeSecretRemoval,
        ) -> Result<RuntimeSecretTargetOutcome, Self::Error> {
            self.removals
                .lock()
                .unwrap()
                .push((*request.operation_id(), *request.installation_id()));
            Ok(self
                .outcome
                .lock()
                .unwrap()
                .unwrap_or(RuntimeSecretTargetOutcome::Applied))
        }
    }

    fn test_root() -> PathBuf {
        unique_test_root('v')
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

    #[test]
    fn permits_only_owner_or_owner_group_socket_access() {
        let owner_only =
            UnixRuntimeSecretServerConfig::new("/run/rumahl/secrets.sock", 1000).unwrap();
        assert_eq!(owner_only.socket_mode(), 0o600);
        let shared = owner_only.with_socket_mode(0o660).unwrap();
        assert_eq!(shared.socket_mode(), 0o660);
        assert_eq!(
            UnixRuntimeSecretServerConfig::new("/run/rumahl/secrets.sock", 1000)
                .unwrap()
                .with_socket_mode(0o666)
                .unwrap_err(),
            UnixRuntimeSecretServerConfigError::InvalidSocketMode
        );
    }

    #[test]
    fn receives_and_validates_oidc_delivery() {
        let root = test_root();
        let socket_path = root.join("runtime.sock");
        let target = RecordingTarget::default();
        let server = UnixRuntimeSecretServer::bind(
            UnixRuntimeSecretServerConfig::new(&socket_path, current_uid()).unwrap(),
            target.clone(),
        )
        .unwrap();
        let operation_id = AppOperationId::new();
        let app = installed_app();
        let client_id = OidcClientId::generate().unwrap();
        let secret = OidcClientSecret::generate().unwrap();
        let expected_secret = secret.encode();

        let worker = thread::spawn(move || server.serve_once());
        let delivery = UnixRuntimeSecretDelivery::new(
            UnixRuntimeSecretDeliveryConfig::new(&socket_path, current_uid()).unwrap(),
        );
        delivery
            .deliver_oidc_client_secret(&operation_id, &app, &client_id, &secret)
            .unwrap();
        worker.join().unwrap().unwrap();

        let deliveries = target.deliveries.lock().unwrap();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].0, operation_id);
        assert_eq!(deliveries[0].1, *app.installation_id());
        assert_eq!(deliveries[0].2, client_id.to_string());
        assert_eq!(deliveries[0].3, expected_secret.as_str());
        assert_eq!(
            fs::metadata(&socket_path).unwrap().permissions().mode() & 0o777,
            OWNER_ONLY_SOCKET_MODE
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn maps_target_conflict_to_client_failure() {
        let root = test_root();
        let socket_path = root.join("runtime.sock");
        let target = RecordingTarget::default();
        *target.outcome.lock().unwrap() = Some(RuntimeSecretTargetOutcome::Conflict);
        let server = UnixRuntimeSecretServer::bind(
            UnixRuntimeSecretServerConfig::new(&socket_path, current_uid()).unwrap(),
            target,
        )
        .unwrap();
        let operation_id = AppOperationId::new();
        let installation_id = InstallationId::new();

        let worker = thread::spawn(move || server.serve_once());
        let delivery = UnixRuntimeSecretDelivery::new(
            UnixRuntimeSecretDeliveryConfig::new(&socket_path, current_uid()).unwrap(),
        );
        let error = delivery
            .remove_for_installation(&operation_id, &installation_id)
            .unwrap_err();

        assert!(matches!(
            error,
            crate::UnixRuntimeSecretDeliveryError::ConflictingReplay
        ));
        worker.join().unwrap().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_malformed_domain_values_without_calling_target() {
        let root = test_root();
        let socket_path = root.join("runtime.sock");
        let target = RecordingTarget::default();
        let server = UnixRuntimeSecretServer::bind(
            UnixRuntimeSecretServerConfig::new(&socket_path, current_uid()).unwrap(),
            target.clone(),
        )
        .unwrap();

        let worker = thread::spawn(move || server.serve_once());
        let mut stream = UnixStream::connect(&socket_path).unwrap();
        let invalid_fields: [&[u8]; 2] = [b"not-an-operation-id", b"not-an-installation-id"];
        let mut request = Vec::from(*PROTOCOL_MAGIC);
        request.push(OP_REMOVE_INSTALLATION);
        request.push(REMOVE_FIELD_COUNT);
        for field in invalid_fields {
            request.extend_from_slice(&(field.len() as u32).to_be_bytes());
            request.extend_from_slice(field);
        }
        stream.write_all(&request).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = [0_u8; 5];
        stream.read_exact(&mut response).unwrap();

        assert_eq!(response[4], STATUS_REJECTED);
        assert!(matches!(
            worker.join().unwrap(),
            Err(UnixRuntimeSecretServerError::InvalidRequest)
        ));
        assert!(target.removals.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
