use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumahl_core::{
    AppId, AppIdentity, AppRuntimeInstallationState, AppRuntimeProvider, AppVersion,
    InstallationId, InstalledApp, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
    RuntimeEntrypoint, RuntimeEntrypointId, RuntimeEntrypointTarget, RuntimeKind,
};

use crate::runtime_secrets::peer_uid;

const PROTOCOL_MAGIC: &[u8; 4] = b"RRP1";
const OP_PREPARE: u8 = 1;
const OP_ACTIVATE: u8 = 2;
const OP_STATE: u8 = 3;
const OP_DEACTIVATE: u8 = 4;
const OP_REMOVE: u8 = 5;
const STATUS_OK: u8 = 0;
const STATUS_CONFLICT: u8 = 1;
const STATUS_REJECTED: u8 = 2;
const STATE_ABSENT: u8 = 0;
const STATE_PREPARED: u8 = 1;
const STATE_ACTIVE: u8 = 2;
const RESPONSE_LENGTH: usize = 7;
const BASE_SPEC_FIELD_COUNT: usize = 5;
const ENTRYPOINT_FIELD_COUNT: usize = 3;
const ID_FIELD_COUNT: u8 = 1;
const MAX_ENTRYPOINTS: usize = 64;
const MAX_FIELD_LENGTH: usize = 1024;
const MAX_REQUEST_LENGTH: usize = 64 * 1024;
const OWNER_ONLY_SOCKET_MODE: u32 = 0o600;
const OWNER_GROUP_SOCKET_MODE: u32 = 0o660;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct UnixAppRuntimeProviderConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixAppRuntimeProviderConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
}

#[derive(Debug, Clone)]
pub struct UnixAppRuntimeProvider {
    config: UnixAppRuntimeProviderConfig,
}

#[derive(Debug)]
pub enum UnixAppRuntimeProviderError {
    Connect(io::Error),
    ConfigureConnection(io::Error),
    PeerCredentials(io::Error),
    UnexpectedPeerUid { expected: u32, actual: u32 },
    InvalidRequest,
    Write(io::Error),
    Read(io::Error),
    InvalidResponse,
    ConflictingReplay,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInstallationSpec {
    identity: AppIdentity,
    version: AppVersion,
    runtime: RuntimeDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeControlTargetOutcome {
    Accepted {
        state: AppRuntimeInstallationState,
        changed: bool,
    },
    Conflict,
    Rejected,
}

/// Supervisor-side target for the authenticated runtime-control channel.
///
/// Implementations own the actual container or static-web namespace. They
/// must compare the complete [`RuntimeInstallationSpec`] on replays and return
/// [`RuntimeControlTargetOutcome::Conflict`] if the same installation ID is
/// associated with a different specification.
pub trait RuntimeControlTarget {
    type Error: Error + Send + Sync + 'static;

    fn prepare(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error>;

    fn activate(
        &self,
        spec: RuntimeInstallationSpec,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error>;

    fn installation_state(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error>;

    fn deactivate(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error>;

    fn remove(
        &self,
        installation_id: InstallationId,
    ) -> Result<RuntimeControlTargetOutcome, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct UnixRuntimeControlServerConfig {
    socket_path: PathBuf,
    expected_peer_uid: u32,
    timeout: Duration,
    socket_mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixRuntimeControlServerConfigError {
    SocketPathMustBeAbsolute,
    ZeroTimeout,
    InvalidSocketMode,
}

pub struct UnixRuntimeControlServer<T> {
    config: UnixRuntimeControlServerConfig,
    listener: UnixListener,
    target: T,
}

#[derive(Debug)]
pub enum UnixRuntimeControlServerError {
    Bind(io::Error),
    SetSocketPermissions(io::Error),
    Accept(io::Error),
    ConfigureConnection(io::Error),
    PeerCredentials(io::Error),
    UnexpectedPeerUid { expected: u32, actual: u32 },
    Read(io::Error),
    InvalidRequest,
    Target,
    InvalidTargetOutcome,
    Write(io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeControlResponse {
    state: AppRuntimeInstallationState,
    changed: bool,
}

enum ParsedRequest {
    Prepare(RuntimeInstallationSpec),
    Activate(RuntimeInstallationSpec),
    State(InstallationId),
    Deactivate(InstallationId),
    Remove(InstallationId),
}

impl UnixAppRuntimeProviderConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixAppRuntimeProviderConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixAppRuntimeProviderConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixAppRuntimeProviderConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixAppRuntimeProviderConfigError::ZeroTimeout);
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

impl UnixAppRuntimeProvider {
    pub fn new(config: UnixAppRuntimeProviderConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &UnixAppRuntimeProviderConfig {
        &self.config
    }

    fn exchange(
        &self,
        operation: u8,
        fields: &[Vec<u8>],
    ) -> Result<RuntimeControlResponse, UnixAppRuntimeProviderError> {
        let request = encode_request(operation, fields)?;
        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(UnixAppRuntimeProviderError::Connect)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixAppRuntimeProviderError::ConfigureConnection)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixAppRuntimeProviderError::ConfigureConnection)?;

        let actual_uid = peer_uid(&stream).map_err(UnixAppRuntimeProviderError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixAppRuntimeProviderError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        stream
            .write_all(&request)
            .map_err(UnixAppRuntimeProviderError::Write)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(UnixAppRuntimeProviderError::Write)?;

        let mut response = [0_u8; RESPONSE_LENGTH];
        stream
            .read_exact(&mut response)
            .map_err(UnixAppRuntimeProviderError::Read)?;
        if &response[..PROTOCOL_MAGIC.len()] != PROTOCOL_MAGIC {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }

        match response[4] {
            STATUS_OK => Ok(RuntimeControlResponse {
                state: decode_state(response[5])
                    .ok_or(UnixAppRuntimeProviderError::InvalidResponse)?,
                changed: match response[6] {
                    0 => false,
                    1 => true,
                    _ => return Err(UnixAppRuntimeProviderError::InvalidResponse),
                },
            }),
            STATUS_CONFLICT => Err(UnixAppRuntimeProviderError::ConflictingReplay),
            STATUS_REJECTED => Err(UnixAppRuntimeProviderError::Rejected),
            _ => Err(UnixAppRuntimeProviderError::InvalidResponse),
        }
    }

    fn exchange_spec(
        &self,
        operation: u8,
        app: &InstalledApp,
    ) -> Result<RuntimeControlResponse, UnixAppRuntimeProviderError> {
        self.exchange(operation, &encode_spec(app)?)
    }

    fn exchange_installation(
        &self,
        operation: u8,
        installation_id: &InstallationId,
    ) -> Result<RuntimeControlResponse, UnixAppRuntimeProviderError> {
        self.exchange(operation, &[installation_id.to_string().into_bytes()])
    }
}

impl AppRuntimeProvider for UnixAppRuntimeProvider {
    type Error = UnixAppRuntimeProviderError;

    fn prepare_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        let response = self.exchange_spec(OP_PREPARE, app)?;
        if !matches!(
            response.state,
            AppRuntimeInstallationState::Prepared | AppRuntimeInstallationState::Active
        ) {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }
        Ok(())
    }

    fn activate_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        let response = self.exchange_spec(OP_ACTIVATE, app)?;
        if response.state != AppRuntimeInstallationState::Active {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }
        Ok(())
    }

    fn installation_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppRuntimeInstallationState, Self::Error> {
        let response = self.exchange_installation(OP_STATE, installation_id)?;
        if response.changed {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }
        Ok(response.state)
    }

    fn deactivate_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<bool, Self::Error> {
        let response = self.exchange_installation(OP_DEACTIVATE, installation_id)?;
        if response.state == AppRuntimeInstallationState::Active {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }
        Ok(response.changed)
    }

    fn remove_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error> {
        let response = self.exchange_installation(OP_REMOVE, installation_id)?;
        if response.state != AppRuntimeInstallationState::Absent {
            return Err(UnixAppRuntimeProviderError::InvalidResponse);
        }
        Ok(response.changed)
    }
}

impl RuntimeInstallationSpec {
    pub fn new(identity: AppIdentity, version: AppVersion, runtime: RuntimeDescriptor) -> Self {
        Self {
            identity,
            version,
            runtime,
        }
    }

    pub fn identity(&self) -> &AppIdentity {
        &self.identity
    }

    pub fn version(&self) -> AppVersion {
        self.version
    }

    pub fn runtime(&self) -> &RuntimeDescriptor {
        &self.runtime
    }
}

impl UnixRuntimeControlServerConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
    ) -> Result<Self, UnixRuntimeControlServerConfigError> {
        Self::with_timeout(socket_path, expected_peer_uid, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        socket_path: impl Into<PathBuf>,
        expected_peer_uid: u32,
        timeout: Duration,
    ) -> Result<Self, UnixRuntimeControlServerConfigError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() {
            return Err(UnixRuntimeControlServerConfigError::SocketPathMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(UnixRuntimeControlServerConfigError::ZeroTimeout);
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

    /// Allows a service manager to grant one trusted group access while peer
    /// credentials still authenticate the exact platform UID.
    pub fn with_socket_mode(
        mut self,
        socket_mode: u32,
    ) -> Result<Self, UnixRuntimeControlServerConfigError> {
        if !matches!(
            socket_mode,
            OWNER_ONLY_SOCKET_MODE | OWNER_GROUP_SOCKET_MODE
        ) {
            return Err(UnixRuntimeControlServerConfigError::InvalidSocketMode);
        }
        self.socket_mode = socket_mode;
        Ok(self)
    }

    pub fn socket_mode(&self) -> u32 {
        self.socket_mode
    }
}

impl<T> UnixRuntimeControlServer<T>
where
    T: RuntimeControlTarget,
{
    /// Binds a new owner-only socket without removing an existing path.
    pub fn bind(
        config: UnixRuntimeControlServerConfig,
        target: T,
    ) -> Result<Self, UnixRuntimeControlServerError> {
        let listener =
            UnixListener::bind(&config.socket_path).map_err(UnixRuntimeControlServerError::Bind)?;
        if let Err(error) = fs::set_permissions(
            &config.socket_path,
            fs::Permissions::from_mode(config.socket_mode),
        ) {
            drop(listener);
            let _ = fs::remove_file(&config.socket_path);
            return Err(UnixRuntimeControlServerError::SetSocketPermissions(error));
        }

        Ok(Self {
            config,
            listener,
            target,
        })
    }

    pub fn config(&self) -> &UnixRuntimeControlServerConfig {
        &self.config
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    /// Accepts and handles one authenticated control request.
    pub fn serve_once(&self) -> Result<(), UnixRuntimeControlServerError> {
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(UnixRuntimeControlServerError::Accept)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeControlServerError::ConfigureConnection)?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(UnixRuntimeControlServerError::ConfigureConnection)?;

        let actual_uid =
            peer_uid(&stream).map_err(UnixRuntimeControlServerError::PeerCredentials)?;
        if actual_uid != self.config.expected_peer_uid {
            return Err(UnixRuntimeControlServerError::UnexpectedPeerUid {
                expected: self.config.expected_peer_uid,
                actual: actual_uid,
            });
        }

        let request = match parse_request(&mut stream) {
            Ok(request) => request,
            Err(error) => {
                write_response(&mut stream, RuntimeControlTargetOutcome::Rejected)
                    .map_err(UnixRuntimeControlServerError::Write)?;
                return Err(error);
            }
        };
        let operation = request.operation();

        let outcome = match request {
            ParsedRequest::Prepare(spec) => self.target.prepare(spec),
            ParsedRequest::Activate(spec) => self.target.activate(spec),
            ParsedRequest::State(installation_id) => {
                self.target.installation_state(installation_id)
            }
            ParsedRequest::Deactivate(installation_id) => self.target.deactivate(installation_id),
            ParsedRequest::Remove(installation_id) => self.target.remove(installation_id),
        };

        match outcome {
            Ok(outcome) => {
                if !target_outcome_is_valid(operation, &outcome) {
                    write_response(&mut stream, RuntimeControlTargetOutcome::Rejected)
                        .map_err(UnixRuntimeControlServerError::Write)?;
                    return Err(UnixRuntimeControlServerError::InvalidTargetOutcome);
                }
                write_response(&mut stream, outcome).map_err(UnixRuntimeControlServerError::Write)
            }
            Err(_) => {
                write_response(&mut stream, RuntimeControlTargetOutcome::Rejected)
                    .map_err(UnixRuntimeControlServerError::Write)?;
                Err(UnixRuntimeControlServerError::Target)
            }
        }
    }
}

impl ParsedRequest {
    fn operation(&self) -> u8 {
        match self {
            Self::Prepare(_) => OP_PREPARE,
            Self::Activate(_) => OP_ACTIVATE,
            Self::State(_) => OP_STATE,
            Self::Deactivate(_) => OP_DEACTIVATE,
            Self::Remove(_) => OP_REMOVE,
        }
    }
}

fn encode_spec(app: &InstalledApp) -> Result<Vec<Vec<u8>>, UnixAppRuntimeProviderError> {
    let runtime = app.manifest().runtime();
    if runtime.entrypoints().len() > MAX_ENTRYPOINTS {
        return Err(UnixAppRuntimeProviderError::InvalidRequest);
    }

    let mut fields = Vec::with_capacity(
        BASE_SPEC_FIELD_COUNT + runtime.entrypoints().len() * ENTRYPOINT_FIELD_COUNT,
    );
    fields.push(app.installation_id().to_string().into_bytes());
    fields.push(app.identity().app_id().as_str().as_bytes().to_vec());
    fields.push(app.identity().publisher_id().as_str().as_bytes().to_vec());
    fields.push(app.manifest().version().to_string().into_bytes());
    fields.push(runtime_kind_name(runtime.kind()).as_bytes().to_vec());

    for entrypoint in runtime.entrypoints() {
        fields.push(entrypoint.id().as_str().as_bytes().to_vec());
        match entrypoint.target() {
            RuntimeEntrypointTarget::WebAsset(path) => {
                fields.push(b"web-asset".to_vec());
                fields.push(path.as_str().as_bytes().to_vec());
            }
            RuntimeEntrypointTarget::ContainerArtifact(path) => {
                fields.push(b"container-artifact".to_vec());
                fields.push(path.as_str().as_bytes().to_vec());
            }
            RuntimeEntrypointTarget::Endpoint(endpoint) => {
                fields.push(b"endpoint".to_vec());
                fields.push(endpoint.as_str().as_bytes().to_vec());
            }
        }
    }

    Ok(fields)
}

fn encode_request(
    operation: u8,
    fields: &[Vec<u8>],
) -> Result<Vec<u8>, UnixAppRuntimeProviderError> {
    let field_count =
        u8::try_from(fields.len()).map_err(|_| UnixAppRuntimeProviderError::InvalidRequest)?;
    let mut request = Vec::new();
    request.extend_from_slice(PROTOCOL_MAGIC);
    request.push(operation);
    request.push(field_count);

    for field in fields {
        if field.is_empty() || field.len() > MAX_FIELD_LENGTH {
            return Err(UnixAppRuntimeProviderError::InvalidRequest);
        }
        let length =
            u32::try_from(field.len()).map_err(|_| UnixAppRuntimeProviderError::InvalidRequest)?;
        request.extend_from_slice(&length.to_be_bytes());
        request.extend_from_slice(field);
        if request.len() > MAX_REQUEST_LENGTH {
            return Err(UnixAppRuntimeProviderError::InvalidRequest);
        }
    }

    Ok(request)
}

fn parse_request(stream: &mut UnixStream) -> Result<ParsedRequest, UnixRuntimeControlServerError> {
    let mut header = [0_u8; 6];
    stream
        .read_exact(&mut header)
        .map_err(UnixRuntimeControlServerError::Read)?;
    if &header[..4] != PROTOCOL_MAGIC {
        return Err(UnixRuntimeControlServerError::InvalidRequest);
    }

    let operation = header[4];
    let field_count = usize::from(header[5]);
    match operation {
        OP_PREPARE | OP_ACTIVATE => {
            if field_count < BASE_SPEC_FIELD_COUNT
                || !(field_count - BASE_SPEC_FIELD_COUNT).is_multiple_of(ENTRYPOINT_FIELD_COUNT)
                || (field_count - BASE_SPEC_FIELD_COUNT) / ENTRYPOINT_FIELD_COUNT > MAX_ENTRYPOINTS
            {
                return Err(UnixRuntimeControlServerError::InvalidRequest);
            }
        }
        OP_STATE | OP_DEACTIVATE | OP_REMOVE => {
            if header[5] != ID_FIELD_COUNT {
                return Err(UnixRuntimeControlServerError::InvalidRequest);
            }
        }
        _ => return Err(UnixRuntimeControlServerError::InvalidRequest),
    }

    let mut total_length = header.len();
    let mut fields = Vec::with_capacity(field_count);
    for _ in 0..field_count {
        let mut length_bytes = [0_u8; 4];
        stream
            .read_exact(&mut length_bytes)
            .map_err(UnixRuntimeControlServerError::Read)?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
        total_length = total_length
            .checked_add(length_bytes.len())
            .and_then(|length_so_far| length_so_far.checked_add(length))
            .ok_or(UnixRuntimeControlServerError::InvalidRequest)?;
        if length == 0 || length > MAX_FIELD_LENGTH || total_length > MAX_REQUEST_LENGTH {
            return Err(UnixRuntimeControlServerError::InvalidRequest);
        }

        let mut field = vec![0_u8; length];
        stream
            .read_exact(&mut field)
            .map_err(UnixRuntimeControlServerError::Read)?;
        fields.push(field);
    }

    let mut trailing = [0_u8; 1];
    match stream.read(&mut trailing) {
        Ok(0) => {}
        Ok(_) => return Err(UnixRuntimeControlServerError::InvalidRequest),
        Err(error) => return Err(UnixRuntimeControlServerError::Read(error)),
    }

    match operation {
        OP_PREPARE => parse_spec(&fields).map(ParsedRequest::Prepare),
        OP_ACTIVATE => parse_spec(&fields).map(ParsedRequest::Activate),
        OP_STATE => parse_installation_id(&fields).map(ParsedRequest::State),
        OP_DEACTIVATE => parse_installation_id(&fields).map(ParsedRequest::Deactivate),
        OP_REMOVE => parse_installation_id(&fields).map(ParsedRequest::Remove),
        _ => unreachable!("operation was validated above"),
    }
}

fn parse_spec(
    fields: &[Vec<u8>],
) -> Result<RuntimeInstallationSpec, UnixRuntimeControlServerError> {
    let installation_id = InstallationId::parse(field_as_str(&fields[0])?)
        .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
    let app_id = AppId::parse(field_as_str(&fields[1])?)
        .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
    let publisher_id = PublisherId::parse(field_as_str(&fields[2])?)
        .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
    let version = AppVersion::parse(field_as_str(&fields[3])?)
        .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
    let kind = parse_runtime_kind(field_as_str(&fields[4])?)?;
    let mut runtime = RuntimeDescriptor::new(kind);

    for entrypoint in fields[BASE_SPEC_FIELD_COUNT..].chunks_exact(ENTRYPOINT_FIELD_COUNT) {
        let id = RuntimeEntrypointId::parse(field_as_str(&entrypoint[0])?)
            .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
        let target_kind = field_as_str(&entrypoint[1])?;
        let target = field_as_str(&entrypoint[2])?;
        let entrypoint = match target_kind {
            "web-asset" => RuntimeEntrypoint::web_asset(
                id,
                PackagePath::parse(target)
                    .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?,
            ),
            "container-artifact" => RuntimeEntrypoint::container_artifact(
                id,
                PackagePath::parse(target)
                    .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?,
            ),
            "endpoint" => RuntimeEntrypoint::endpoint(
                id,
                RuntimeEndpointId::parse(target)
                    .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?,
            ),
            _ => return Err(UnixRuntimeControlServerError::InvalidRequest),
        };
        runtime
            .add_entrypoint(entrypoint)
            .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)?;
    }

    Ok(RuntimeInstallationSpec::new(
        AppIdentity::new(app_id, installation_id, publisher_id),
        version,
        runtime,
    ))
}

fn parse_installation_id(
    fields: &[Vec<u8>],
) -> Result<InstallationId, UnixRuntimeControlServerError> {
    InstallationId::parse(field_as_str(&fields[0])?)
        .map_err(|_| UnixRuntimeControlServerError::InvalidRequest)
}

fn field_as_str(field: &[u8]) -> Result<&str, UnixRuntimeControlServerError> {
    std::str::from_utf8(field).map_err(|_| UnixRuntimeControlServerError::InvalidRequest)
}

fn runtime_kind_name(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Web => "web",
        RuntimeKind::Container => "container",
        RuntimeKind::Native => "native",
    }
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, UnixRuntimeControlServerError> {
    match value {
        "web" => Ok(RuntimeKind::Web),
        "container" => Ok(RuntimeKind::Container),
        "native" => Ok(RuntimeKind::Native),
        _ => Err(UnixRuntimeControlServerError::InvalidRequest),
    }
}

fn encode_state(state: AppRuntimeInstallationState) -> u8 {
    match state {
        AppRuntimeInstallationState::Absent => STATE_ABSENT,
        AppRuntimeInstallationState::Prepared => STATE_PREPARED,
        AppRuntimeInstallationState::Active => STATE_ACTIVE,
    }
}

fn decode_state(value: u8) -> Option<AppRuntimeInstallationState> {
    match value {
        STATE_ABSENT => Some(AppRuntimeInstallationState::Absent),
        STATE_PREPARED => Some(AppRuntimeInstallationState::Prepared),
        STATE_ACTIVE => Some(AppRuntimeInstallationState::Active),
        _ => None,
    }
}

fn target_outcome_is_valid(operation: u8, outcome: &RuntimeControlTargetOutcome) -> bool {
    let RuntimeControlTargetOutcome::Accepted { state, changed } = outcome else {
        return true;
    };

    match operation {
        OP_PREPARE => matches!(
            state,
            AppRuntimeInstallationState::Prepared | AppRuntimeInstallationState::Active
        ),
        OP_ACTIVATE => *state == AppRuntimeInstallationState::Active,
        OP_STATE => !changed,
        OP_DEACTIVATE => *state != AppRuntimeInstallationState::Active,
        OP_REMOVE => *state == AppRuntimeInstallationState::Absent,
        _ => false,
    }
}

fn write_response(stream: &mut UnixStream, outcome: RuntimeControlTargetOutcome) -> io::Result<()> {
    let (status, state, changed) = match outcome {
        RuntimeControlTargetOutcome::Accepted { state, changed } => {
            (STATUS_OK, encode_state(state), u8::from(changed))
        }
        RuntimeControlTargetOutcome::Conflict => (STATUS_CONFLICT, STATE_ABSENT, 0),
        RuntimeControlTargetOutcome::Rejected => (STATUS_REJECTED, STATE_ABSENT, 0),
    };
    stream.write_all(&[
        PROTOCOL_MAGIC[0],
        PROTOCOL_MAGIC[1],
        PROTOCOL_MAGIC[2],
        PROTOCOL_MAGIC[3],
        status,
        state,
        changed,
    ])
}

impl fmt::Display for UnixAppRuntimeProviderConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "runtime provider socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "runtime provider timeout must be non-zero"),
        }
    }
}

impl Error for UnixAppRuntimeProviderConfigError {}

impl fmt::Display for UnixAppRuntimeProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(_) => write!(f, "runtime provider socket connection failed"),
            Self::ConfigureConnection(_) => {
                write!(f, "runtime provider socket setup failed")
            }
            Self::PeerCredentials(_) => {
                write!(f, "runtime provider peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "runtime supervisor UID {actual} does not match expected UID {expected}"
            ),
            Self::InvalidRequest => write!(f, "runtime provider request is invalid"),
            Self::Write(_) => write!(f, "runtime provider request failed"),
            Self::Read(_) => write!(f, "runtime provider response failed"),
            Self::InvalidResponse => write!(f, "runtime provider response is invalid"),
            Self::ConflictingReplay => {
                write!(
                    f,
                    "runtime operation conflicts with an earlier installation"
                )
            }
            Self::Rejected => write!(f, "runtime operation was rejected"),
        }
    }
}

impl Error for UnixAppRuntimeProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error)
            | Self::ConfigureConnection(error)
            | Self::PeerCredentials(error)
            | Self::Write(error)
            | Self::Read(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for UnixRuntimeControlServerConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketPathMustBeAbsolute => {
                write!(f, "runtime control server socket path must be absolute")
            }
            Self::ZeroTimeout => write!(f, "runtime control server timeout must be non-zero"),
            Self::InvalidSocketMode => {
                write!(f, "runtime control server socket mode must be 0600 or 0660")
            }
        }
    }
}

impl Error for UnixRuntimeControlServerConfigError {}

impl fmt::Display for UnixRuntimeControlServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(_) => write!(f, "runtime control server socket bind failed"),
            Self::SetSocketPermissions(_) => {
                write!(f, "runtime control server socket permission setup failed")
            }
            Self::Accept(_) => write!(f, "runtime control server accept failed"),
            Self::ConfigureConnection(_) => {
                write!(f, "runtime control server connection setup failed")
            }
            Self::PeerCredentials(_) => {
                write!(f, "runtime control server peer authentication failed")
            }
            Self::UnexpectedPeerUid { expected, actual } => write!(
                f,
                "runtime provider UID {actual} does not match expected UID {expected}"
            ),
            Self::Read(_) => write!(f, "runtime control request read failed"),
            Self::InvalidRequest => write!(f, "runtime control request is invalid"),
            Self::Target => write!(f, "runtime control target failed"),
            Self::InvalidTargetOutcome => {
                write!(f, "runtime control target returned invalid state")
            }
            Self::Write(_) => write!(f, "runtime control response write failed"),
        }
    }
}

impl Error for UnixRuntimeControlServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bind(error)
            | Self::SetSocketPermissions(error)
            | Self::Accept(error)
            | Self::ConfigureConnection(error)
            | Self::PeerCredentials(error)
            | Self::Read(error)
            | Self::Write(error) => Some(error),
            Self::UnexpectedPeerUid { .. }
            | Self::InvalidRequest
            | Self::Target
            | Self::InvalidTargetOutcome => None,
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
    use rumahl_core::{AppManifest, AppManifestValidator};

    #[derive(Default)]
    struct MemoryTarget {
        runtime: Mutex<Option<(RuntimeInstallationSpec, AppRuntimeInstallationState)>>,
    }

    impl RuntimeControlTarget for Arc<MemoryTarget> {
        type Error = Infallible;

        fn prepare(
            &self,
            spec: RuntimeInstallationSpec,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let mut runtime = self.runtime.lock().unwrap();
            match runtime.as_ref() {
                Some((existing, state)) if existing == &spec => {
                    Ok(RuntimeControlTargetOutcome::Accepted {
                        state: *state,
                        changed: false,
                    })
                }
                Some(_) => Ok(RuntimeControlTargetOutcome::Conflict),
                None => {
                    *runtime = Some((spec, AppRuntimeInstallationState::Prepared));
                    Ok(RuntimeControlTargetOutcome::Accepted {
                        state: AppRuntimeInstallationState::Prepared,
                        changed: true,
                    })
                }
            }
        }

        fn activate(
            &self,
            spec: RuntimeInstallationSpec,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let mut runtime = self.runtime.lock().unwrap();
            let Some((existing, state)) = runtime.as_mut() else {
                return Ok(RuntimeControlTargetOutcome::Rejected);
            };
            if existing != &spec {
                return Ok(RuntimeControlTargetOutcome::Conflict);
            }
            let changed = *state != AppRuntimeInstallationState::Active;
            *state = AppRuntimeInstallationState::Active;
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: *state,
                changed,
            })
        }

        fn installation_state(
            &self,
            installation_id: InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let runtime = self.runtime.lock().unwrap();
            let state = runtime
                .as_ref()
                .filter(|(spec, _)| spec.identity().installation_id() == &installation_id)
                .map(|(_, state)| *state)
                .unwrap_or(AppRuntimeInstallationState::Absent);
            Ok(RuntimeControlTargetOutcome::Accepted {
                state,
                changed: false,
            })
        }

        fn deactivate(
            &self,
            installation_id: InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let mut runtime = self.runtime.lock().unwrap();
            let Some((spec, state)) = runtime.as_mut() else {
                return Ok(RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Absent,
                    changed: false,
                });
            };
            if spec.identity().installation_id() != &installation_id {
                return Ok(RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Absent,
                    changed: false,
                });
            }
            let changed = *state == AppRuntimeInstallationState::Active;
            *state = AppRuntimeInstallationState::Prepared;
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: *state,
                changed,
            })
        }

        fn remove(
            &self,
            installation_id: InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let mut runtime = self.runtime.lock().unwrap();
            let Some((spec, state)) = runtime.as_ref() else {
                return Ok(RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Absent,
                    changed: false,
                });
            };
            if spec.identity().installation_id() != &installation_id {
                return Ok(RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Absent,
                    changed: false,
                });
            }
            if *state == AppRuntimeInstallationState::Active {
                return Ok(RuntimeControlTargetOutcome::Rejected);
            }
            *runtime = None;
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Absent,
                changed: true,
            })
        }
    }

    fn test_root() -> PathBuf {
        unique_test_root('p')
    }

    fn current_uid() -> u32 {
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() }
    }

    fn installed_app(version: AppVersion) -> InstalledApp {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            version,
            "Notes",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn permits_only_owner_or_owner_group_socket_access() {
        let owner_only =
            UnixRuntimeControlServerConfig::new("/run/rumahl/control.sock", 1000).unwrap();
        assert_eq!(owner_only.socket_mode(), 0o600);
        let shared = owner_only.with_socket_mode(0o660).unwrap();
        assert_eq!(shared.socket_mode(), 0o660);
        assert_eq!(
            UnixRuntimeControlServerConfig::new("/run/rumahl/control.sock", 1000)
                .unwrap()
                .with_socket_mode(0o666)
                .unwrap_err(),
            UnixRuntimeControlServerConfigError::InvalidSocketMode
        );
    }

    #[test]
    fn controls_runtime_through_authenticated_replay_safe_server() {
        let root = test_root();
        let socket_path = root.join("control.sock");
        let target = Arc::new(MemoryTarget::default());
        let server = UnixRuntimeControlServer::bind(
            UnixRuntimeControlServerConfig::new(&socket_path, current_uid()).unwrap(),
            Arc::clone(&target),
        )
        .unwrap();
        let server_thread = thread::spawn(move || {
            for _ in 0..8 {
                server.serve_once().unwrap();
            }
        });
        let provider = UnixAppRuntimeProvider::new(
            UnixAppRuntimeProviderConfig::new(&socket_path, current_uid()).unwrap(),
        );
        let app = installed_app(AppVersion::new(1, 0, 0));

        assert_eq!(
            provider.installation_state(app.installation_id()).unwrap(),
            AppRuntimeInstallationState::Absent
        );
        provider.prepare_installation(&app).unwrap();
        provider.prepare_installation(&app).unwrap();
        assert_eq!(
            provider.installation_state(app.installation_id()).unwrap(),
            AppRuntimeInstallationState::Prepared
        );
        provider.activate_installation(&app).unwrap();
        provider.activate_installation(&app).unwrap();
        assert!(
            provider
                .deactivate_installation(app.installation_id())
                .unwrap()
        );
        assert!(provider.remove_installation(app.installation_id()).unwrap());

        server_thread.join().unwrap();
        assert!(target.runtime.lock().unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_conflicting_runtime_for_same_installation() {
        let root = test_root();
        let socket_path = root.join("control.sock");
        let target = Arc::new(MemoryTarget::default());
        let server = UnixRuntimeControlServer::bind(
            UnixRuntimeControlServerConfig::new(&socket_path, current_uid()).unwrap(),
            Arc::clone(&target),
        )
        .unwrap();
        let server_thread = thread::spawn(move || {
            server.serve_once().unwrap();
            server.serve_once().unwrap();
        });
        let provider = UnixAppRuntimeProvider::new(
            UnixAppRuntimeProviderConfig::new(&socket_path, current_uid()).unwrap(),
        );
        let app = installed_app(AppVersion::new(1, 0, 0));
        provider.prepare_installation(&app).unwrap();

        let mut conflicting_fields = encode_spec(&app).unwrap();
        conflicting_fields[3] = AppVersion::new(2, 0, 0).to_string().into_bytes();
        let error = provider
            .exchange(OP_PREPARE, &conflicting_fields)
            .unwrap_err();

        assert!(matches!(
            error,
            UnixAppRuntimeProviderError::ConflictingReplay
        ));
        server_thread.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unexpected_supervisor_uid_before_sending_request() {
        let root = test_root();
        let socket_path = root.join("control.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            request
        });
        let unexpected_uid = current_uid().checked_add(1).unwrap();
        let provider = UnixAppRuntimeProvider::new(
            UnixAppRuntimeProviderConfig::new(&socket_path, unexpected_uid).unwrap(),
        );

        let error = provider
            .installation_state(&InstallationId::new())
            .unwrap_err();

        assert!(matches!(
            error,
            UnixAppRuntimeProviderError::UnexpectedPeerUid { .. }
        ));
        assert!(server_thread.join().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
