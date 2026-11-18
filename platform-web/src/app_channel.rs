//! Persistent app <-> OS channel transport.
//!
//! A running app instance (typically an always-on service) connects to a
//! loopback endpoint and authenticates with a per-installation token. The OS
//! then registers a live [`RuntimeChannel`] for it, so OS events reach the app
//! even while it has no window open. Frames are JSON lines, versioned by
//! `rumahl.channel.v1`.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    AppAccessError, AppFileAccess, AppFileAction, AppFileDeny, AppFileOutcome, ShellIdentity,
};
use rand::RngCore;
use rumahl_core::{
    AuditAction, AuditLog, AuditOutcome, InstallationId, RuntimeCapabilityExecution,
    RuntimeCapabilityResult, RuntimeChannel, RuntimeChannelError, RuntimeChannelRegistry,
    RuntimeEvent, SessionId, UserId,
};
use serde::{Deserialize, Serialize};

pub const CHANNEL_PROTOCOL: &str = "rumahl.channel.v1";

/// How long a capability invocation waits for the provider's result.
const EXECUTE_TIMEOUT: Duration = Duration::from_secs(5);

/// One JSON-line frame on the persistent channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChannelFrame {
    /// The app's first frame: identifies and authenticates the installation.
    #[serde(rename_all = "camelCase")]
    Hello {
        protocol: String,
        installation_id: String,
        token: String,
    },
    /// An event the OS pushes to the app.
    #[serde(rename_all = "camelCase")]
    Event {
        topic: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resource: Option<String>,
    },
    /// A capability invocation the OS routes to the app that provides it.
    #[serde(rename_all = "camelCase")]
    Capability {
        id: String,
        capability: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resource: Option<String>,
    },
    /// The provider's result for a capability invocation.
    #[serde(rename_all = "camelCase")]
    CapabilityResult {
        id: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },
    /// A user-scoped file access request from the running app, bound to the
    /// capability invocation the user made. The OS authorizes it with that
    /// invocation's user and session — never with the app's own claim.
    #[serde(rename_all = "camelCase")]
    FileRequest {
        id: String,
        invocation: String,
        action: String,
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_base64: Option<String>,
    },
    /// The OS's answer to a file request.
    #[serde(rename_all = "camelCase")]
    FileResponse {
        id: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_base64: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Executes one file request for a channel app, authorized with the user and
/// session of the capability invocation that is still in flight.
pub trait ChannelFiles: Send + Sync {
    fn access(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError>;
}

impl ChannelFrame {
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn decode(line: &str) -> Option<Self> {
        serde_json::from_str(line).ok()
    }
}

/// Verifies a channel `hello` against the trusted per-installation token store.
pub trait ChannelAuthenticator: Send + Sync {
    /// Returns the installation the token authenticates, or `None` when the
    /// token is unknown or expired.
    fn authenticate(&self, installation: &str, token: &str) -> Option<InstallationId>;
}

/// In-memory store of per-installation channel tokens. The token is delivered
/// to the running app out of band; only the store can authenticate it.
#[derive(Default)]
pub struct ChannelTokenStore {
    tokens: Mutex<HashMap<String, InstallationId>>,
}

impl ChannelTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issues a fresh 256-bit token for an installation, replacing any prior one.
    pub fn issue(&self, installation: InstallationId) -> String {
        let mut bytes = [0_u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.retain(|_, value| value != &installation);
            tokens.insert(token.clone(), installation);
        }
        token
    }

    pub fn revoke(&self, installation: &InstallationId) {
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.retain(|_, value| value != installation);
        }
    }
}

impl ChannelAuthenticator for ChannelTokenStore {
    fn authenticate(&self, installation: &str, token: &str) -> Option<InstallationId> {
        let found = self.tokens.lock().ok()?.get(token).copied()?;
        (found.to_string() == installation).then_some(found)
    }
}

/// Shared state for one app connection: the writer plus in-flight capability
/// requests awaiting their result frame.
pub struct Connection<W: Write + Send> {
    writer: Mutex<W>,
    pending: Mutex<HashMap<String, mpsc::Sender<Result<String, ()>>>>,
    /// Who is behind each in-flight capability invocation. This is the only
    /// identity an app may use for user-scoped work.
    invocations: Mutex<HashMap<String, (UserId, SessionId)>>,
    sequence: AtomicU64,
}

impl<W: Write + Send> Connection<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
            pending: Mutex::new(HashMap::new()),
            invocations: Mutex::new(HashMap::new()),
            sequence: AtomicU64::new(0),
        }
    }

    /// Sends a frame the OS initiates outside a capability result.
    pub(crate) fn respond(&self, frame: &ChannelFrame) -> Result<(), RuntimeChannelError> {
        self.write_frame(frame)
    }

    /// The identity of the user whose invocation is still in flight.
    fn invocation_identity(&self, invocation: &str) -> Option<(UserId, SessionId)> {
        self.invocations
            .lock()
            .ok()
            .and_then(|invocations| invocations.get(invocation).copied())
    }

    fn write_frame(&self, frame: &ChannelFrame) -> Result<(), RuntimeChannelError> {
        let line = frame.encode().map_err(|_| RuntimeChannelError::Rejected)?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| RuntimeChannelError::Unavailable)?;
        writeln!(writer, "{line}").map_err(|_| RuntimeChannelError::Unavailable)?;
        writer.flush().map_err(|_| RuntimeChannelError::Unavailable)
    }

    /// Resolves a pending capability request from a result frame.
    pub fn resolve(&self, frame: ChannelFrame) {
        let ChannelFrame::CapabilityResult { id, ok, result } = frame else {
            return;
        };
        let waiter = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id));
        self.forget_invocation(&id);
        if let Some(waiter) = waiter {
            let _ = waiter.send(if ok {
                Ok(result.unwrap_or_default())
            } else {
                Err(())
            });
        }
    }

    /// Fails every in-flight request when the connection drops, so callers do
    /// not wait for the full timeout.
    pub fn disconnect(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            for (_, waiter) in pending.drain() {
                let _ = waiter.send(Err(()));
            }
        }
    }

    fn request(
        &self,
        execution: &RuntimeCapabilityExecution,
    ) -> Result<RuntimeCapabilityResult, RuntimeChannelError> {
        let id = format!("{:016x}", self.sequence.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| RuntimeChannelError::Unavailable)?
            .insert(id.clone(), sender);
        if let Some(identity) = execution.identity()
            && let Ok(mut invocations) = self.invocations.lock()
        {
            invocations.insert(id.clone(), identity);
        }
        let frame = ChannelFrame::Capability {
            id: id.clone(),
            capability: execution.capability.clone(),
            resource: execution.resource.clone(),
        };
        if let Err(error) = self.write_frame(&frame) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            self.forget_invocation(&id);
            return Err(error);
        }
        match receiver.recv_timeout(EXECUTE_TIMEOUT) {
            Ok(Ok(payload)) => Ok(RuntimeCapabilityResult::new(payload)),
            Ok(Err(())) => Err(RuntimeChannelError::Rejected),
            Err(_) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                self.forget_invocation(&id);
                Err(RuntimeChannelError::Unavailable)
            }
        }
    }

    fn forget_invocation(&self, id: &str) {
        if let Ok(mut invocations) = self.invocations.lock() {
            invocations.remove(id);
        }
    }
}

/// A `RuntimeChannel` over one app connection.
pub struct SocketRuntimeChannel<W: Write + Send> {
    connection: Arc<Connection<W>>,
}

impl<W: Write + Send> SocketRuntimeChannel<W> {
    pub fn new(writer: W) -> Self {
        Self {
            connection: Arc::new(Connection::new(writer)),
        }
    }

    pub fn from_connection(connection: Arc<Connection<W>>) -> Self {
        Self { connection }
    }
}

impl<W: Write + Send> RuntimeChannel for SocketRuntimeChannel<W> {
    fn deliver(&self, event: &RuntimeEvent) -> Result<(), RuntimeChannelError> {
        self.connection.write_frame(&ChannelFrame::Event {
            topic: event.topic.clone(),
            resource: event.resource.clone(),
        })
    }

    fn execute(
        &self,
        execution: &RuntimeCapabilityExecution,
    ) -> Result<RuntimeCapabilityResult, RuntimeChannelError> {
        self.connection.request(execution)
    }
}

/// Largest hello line accepted from an app.
const MAX_HELLO_BYTES: usize = 4096;
/// Largest single frame line accepted. File content is base64 of at most
/// 12 MiB, so this stays comfortably above a full file response.
const MAX_FRAME_BYTES: usize = 18 * 1024 * 1024;
/// Concurrent channel connections. Every connection holds a thread, so a
/// misbehaving local client must not be able to exhaust the process.
const MAX_CONNECTIONS: usize = 64;

/// Reads one newline-terminated line without letting the peer grow the buffer
/// past `limit`. A peer that sends more is refused instead of buffered.
fn read_bounded_line<R: BufRead + ?Sized>(
    reader: &mut R,
    limit: usize,
) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    let read = Read::take(&mut *reader, limit as u64 + 1).read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.len() > limit && !line.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "channel frame exceeds the line limit",
        ));
    }
    Ok(Some(line))
}

/// Reads the first frame of a connection and returns the authenticated
/// installation. Returns `None` for a missing/invalid `hello`.
pub fn read_hello(
    reader: &mut impl BufRead,
    authenticator: &dyn ChannelAuthenticator,
) -> Option<InstallationId> {
    let line = read_bounded_line(reader, MAX_HELLO_BYTES).ok()??;
    let ChannelFrame::Hello {
        protocol,
        installation_id,
        token,
    } = ChannelFrame::decode(line.trim())?
    else {
        return None;
    };
    if protocol != CHANNEL_PROTOCOL {
        return None;
    }
    let installation = authenticator.authenticate(&installation_id, &token)?;
    // The claimed id must be the canonical form of the authenticated id.
    (installation.to_string() == installation_id).then_some(installation)
}

/// Serves one accepted connection: authenticates the hello, registers a live
/// channel for the installation, keeps it registered while the peer stays
/// connected, and unregisters it on disconnect. Call this on a dedicated thread
/// per connection.
/// Decoded bytes an app may write through one channel request; base64 grows
/// the JSON line by a third, so this stays below the file API's 16 MiB.
const MAX_CHANNEL_FILE_BYTES: usize = 12 * 1024 * 1024;

fn encode_file_content(bytes: &[u8]) -> Option<String> {
    (bytes.len() <= MAX_CHANNEL_FILE_BYTES).then(|| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    })
}

fn decode_file_content(encoded: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    (decoded.len() <= MAX_CHANNEL_FILE_BYTES).then_some(decoded)
}

/// Answers one file request with the invocation's identity. Anything the app
/// cannot prove — an unknown invocation, a malformed frame, no executor or a
/// denial from the grants — becomes an error frame, never data.
#[allow(clippy::too_many_arguments)]
fn handle_file_request<W: Write + Send>(
    connection: &Connection<W>,
    installation: InstallationId,
    id: String,
    invocation: String,
    action: String,
    key: String,
    content_base64: Option<String>,
    files: Option<&dyn ChannelFiles>,
) -> ChannelFrame {
    let deny = |error: &str| ChannelFrame::FileResponse {
        id: id.clone(),
        ok: false,
        name: None,
        content_base64: None,
        size: None,
        error: Some(error.to_owned()),
    };
    let Some((user, session)) = connection.invocation_identity(&invocation) else {
        return deny("no-invocation");
    };
    let Some(files) = files else {
        return deny("unavailable");
    };
    if key.is_empty() || key.len() > 255 {
        return deny("invalid");
    }
    let access = match action.as_str() {
        "read" => AppFileAccess {
            action: AppFileAction::Read,
            key,
        },
        "write" => {
            let Some(encoded) = content_base64 else {
                return deny("invalid");
            };
            let Some(bytes) = decode_file_content(&encoded) else {
                return deny("invalid");
            };
            AppFileAccess {
                action: AppFileAction::Write { bytes },
                key,
            }
        }
        _ => return deny("invalid"),
    };
    let identity = ShellIdentity {
        user_id: user,
        session_id: session,
    };
    match files.access(identity, installation, &access) {
        Ok(AppFileOutcome::Content { name, bytes }) => match encode_file_content(&bytes) {
            Some(content_base64) => ChannelFrame::FileResponse {
                id,
                ok: true,
                name: Some(name),
                content_base64: Some(content_base64),
                size: Some(bytes.len() as u64),
                error: None,
            },
            None => deny("too-large"),
        },
        Ok(AppFileOutcome::Allowed { name, size }) => ChannelFrame::FileResponse {
            id,
            ok: true,
            name: Some(name),
            content_base64: None,
            size: Some(size),
            error: None,
        },
        Ok(AppFileOutcome::Missing) => deny("missing"),
        Ok(AppFileOutcome::Denied(reason)) => deny(match reason {
            AppFileDeny::NoGrant => "no-grant",
            AppFileDeny::ApprovalRequired => "approval-required",
            AppFileDeny::OutsideScope => "outside-scope",
        }),
        Err(AppAccessError::Denied) => deny("denied"),
        Err(AppAccessError::Unavailable) => deny("unavailable"),
    }
}

pub fn run_connection<R, W>(
    mut reader: R,
    writer: W,
    registry: Arc<RuntimeChannelRegistry>,
    authenticator: Arc<dyn ChannelAuthenticator>,
    files: Option<Arc<dyn ChannelFiles>>,
    audit: Option<Arc<dyn AuditLog>>,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write + Send + 'static,
{
    let Some(installation) = read_hello(&mut reader, authenticator.as_ref()) else {
        return Ok(());
    };
    let connection = Arc::new(Connection::new(writer));
    let channel = Arc::new(SocketRuntimeChannel::from_connection(Arc::clone(
        &connection,
    )));
    if !registry.register(installation, channel) {
        return Ok(());
    }
    // Dispatch result frames until the app disconnects (0 bytes), sends an
    // oversized line or errors.
    while let Some(line) = read_bounded_line(&mut reader, MAX_FRAME_BYTES)
        .ok()
        .flatten()
    {
        if let Some(frame) = ChannelFrame::decode(line.trim()) {
            match frame {
                ChannelFrame::FileRequest {
                    id,
                    invocation,
                    action,
                    key,
                    content_base64,
                } => {
                    let target = format!("{installation}:{action}:{key}");
                    let response = handle_file_request(
                        &connection,
                        installation,
                        id,
                        invocation,
                        action,
                        key,
                        content_base64,
                        files.as_deref(),
                    );
                    if let Some(audit) = audit.as_ref() {
                        let outcome = match &response {
                            ChannelFrame::FileResponse { ok: true, .. } => AuditOutcome::Success,
                            _ => AuditOutcome::Failure,
                        };
                        if let Ok(event) = rumahl_core::NewAuditEvent::new(
                            rumahl_core::UnixTimestamp::now()
                                .unwrap_or_else(|_| rumahl_core::UnixTimestamp::from_seconds(0)),
                            rumahl_core::AuditActor::App(installation.to_string()),
                            AuditAction::AppFileAccess,
                            outcome,
                        )
                        .with_target(target)
                        {
                            let _ = audit.record(event);
                        }
                    }
                    let _ = connection.respond(&response);
                }
                frame => connection.resolve(frame),
            }
        }
    }
    connection.disconnect();
    registry.unregister(&installation);
    Ok(())
}

/// Accepts app channel connections forever, spawning a thread per connection.
/// Runs until the listener errors (for example when the socket is removed).
pub fn serve_channel_listener(
    listener: UnixListener,
    registry: Arc<RuntimeChannelRegistry>,
    authenticator: Arc<dyn ChannelAuthenticator>,
    files: Option<Arc<dyn ChannelFiles>>,
    audit: Option<Arc<dyn AuditLog>>,
) {
    let live = Arc::new(AtomicU64::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if live.fetch_add(1, Ordering::AcqRel) as usize >= MAX_CONNECTIONS {
            live.fetch_sub(1, Ordering::AcqRel);
            // Dropping the stream closes it without reading an unbounded hello.
            continue;
        }
        let Ok(writer) = stream.try_clone() else {
            live.fetch_sub(1, Ordering::AcqRel);
            continue;
        };
        let reader = BufReader::new(stream);
        let registry = Arc::clone(&registry);
        let authenticator = Arc::clone(&authenticator);
        let files = files.clone();
        let audit = audit.clone();
        let live = Arc::clone(&live);
        std::thread::spawn(move || {
            let _ = run_connection(reader, writer, registry, authenticator, files, audit);
            live.fetch_sub(1, Ordering::AcqRel);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FakeAuthenticator;

    impl ChannelAuthenticator for FakeAuthenticator {
        fn authenticate(&self, installation: &str, token: &str) -> Option<InstallationId> {
            InstallationId::parse(installation)
                .ok()
                .filter(|_| token == "valid-token")
        }
    }

    fn installation() -> InstallationId {
        InstallationId::new()
    }

    #[test]
    fn frames_round_trip_as_tagged_json() {
        let hello = ChannelFrame::Hello {
            protocol: CHANNEL_PROTOCOL.into(),
            installation_id: installation().to_string(),
            token: "valid-token".into(),
        };
        assert_eq!(ChannelFrame::decode(&hello.encode().unwrap()), Some(hello));

        let event = ChannelFrame::Event {
            topic: "rumahl.files.changed".into(),
            resource: Some("rumahl.files/file/document-1".into()),
        };
        let line = event.encode().unwrap();
        assert!(line.contains("\"kind\":\"event\""));
        assert!(line.contains("\"topic\":\"rumahl.files.changed\""));
        assert_eq!(ChannelFrame::decode(&line), Some(event));
        assert_eq!(ChannelFrame::decode("not json"), None);
    }

    #[test]
    fn channel_writes_events_as_json_lines() {
        let sink = SharedWriter::default();
        let channel = SocketRuntimeChannel::new(sink.clone());
        channel
            .deliver(&RuntimeEvent {
                topic: "rumahl.apps.installed".into(),
                resource: None,
            })
            .unwrap();

        let written = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            written,
            "{\"kind\":\"event\",\"topic\":\"rumahl.apps.installed\"}\n"
        );
    }

    #[test]
    fn executes_a_capability_and_awaits_the_result() {
        use std::io::BufReader;
        use std::os::unix::net::UnixStream;
        use std::thread;

        let (server, client) = UnixStream::pair().unwrap();
        let connection = Arc::new(Connection::new(server.try_clone().unwrap()));
        let channel = SocketRuntimeChannel::from_connection(Arc::clone(&connection));

        // OS side: read the provider's result frame and resolve the request.
        let server_reader = server.try_clone().unwrap();
        let resolver = {
            let connection = Arc::clone(&connection);
            thread::spawn(move || {
                let mut lines = BufReader::new(server_reader);
                let mut line = String::new();
                lines.read_line(&mut line).unwrap();
                if let Some(frame) = ChannelFrame::decode(line.trim()) {
                    connection.resolve(frame);
                }
            })
        };

        // App side: read the capability frame and answer with a result.
        let app = thread::spawn(move || {
            let mut stream = client;
            let mut line = String::new();
            {
                let mut lines = BufReader::new(stream.try_clone().unwrap());
                lines.read_line(&mut line).unwrap();
            }
            let ChannelFrame::Capability { id, capability, .. } =
                ChannelFrame::decode(line.trim()).unwrap()
            else {
                panic!("expected a capability frame");
            };
            assert_eq!(capability, "rumahl.search.query");
            let result = ChannelFrame::CapabilityResult {
                id,
                ok: true,
                result: Some("{\"answer\":42}".into()),
            };
            writeln!(stream, "{}", result.encode().unwrap()).unwrap();
            stream.flush().unwrap();
        });

        let outcome = channel
            .execute(&RuntimeCapabilityExecution {
                capability: "rumahl.search.query".into(),
                resource: None,
                user: None,
                session: None,
            })
            .unwrap();
        assert_eq!(outcome.payload(), "{\"answer\":42}");
        app.join().unwrap();
        resolver.join().unwrap();
    }

    #[test]
    fn file_requests_use_the_invocations_identity_and_fail_closed() {
        use rumahl_core::SessionId;
        use rumahl_core::UserId;
        use std::io::BufReader;
        use std::os::unix::net::UnixStream;
        use std::sync::Mutex;
        use std::thread;
        use std::time::Duration;

        #[derive(Default)]
        struct RecordingFiles {
            seen: Mutex<Vec<(ShellIdentity, InstallationId, AppFileAccess)>>,
        }

        impl ChannelFiles for RecordingFiles {
            fn access(
                &self,
                identity: ShellIdentity,
                installation: InstallationId,
                access: &AppFileAccess,
            ) -> Result<AppFileOutcome, AppAccessError> {
                self.seen
                    .lock()
                    .unwrap()
                    .push((identity, installation, access.clone()));
                Ok(AppFileOutcome::Content {
                    name: "notes.txt".into(),
                    bytes: b"hello".to_vec(),
                })
            }
        }

        let (server, mut client) = UnixStream::pair().unwrap();
        let reader = BufReader::new(server.try_clone().unwrap());
        let registry = Arc::new(RuntimeChannelRegistry::new());
        let authenticator: Arc<dyn ChannelAuthenticator> = Arc::new(FakeAuthenticator);
        let files = Arc::new(RecordingFiles::default());
        let files_dyn: Arc<dyn ChannelFiles> = files.clone();
        let registry_for_connection = Arc::clone(&registry);
        let installation = installation();
        let handle = thread::spawn(move || {
            run_connection(
                reader,
                server,
                registry_for_connection,
                authenticator,
                Some(files_dyn),
                None,
            )
            .unwrap()
        });
        client
            .write_all(
                format!(
                    "{}\n",
                    ChannelFrame::Hello {
                        protocol: CHANNEL_PROTOCOL.into(),
                        installation_id: installation.to_string(),
                        token: "valid-token".into(),
                    }
                    .encode()
                    .unwrap()
                )
                .as_bytes(),
            )
            .unwrap();
        client.flush().unwrap();
        let mut waited = 0;
        while !registry.is_registered(&installation) && waited < 200 {
            thread::sleep(Duration::from_millis(5));
            waited += 1;
        }
        assert!(registry.is_registered(&installation));

        // The OS invokes a capability for a user; the app learns the invocation id.
        let user = UserId::new();
        let session = SessionId::new();
        let execution = RuntimeCapabilityExecution {
            capability: "rumahl.notes.save".into(),
            resource: None,
            user: Some(user),
            session: Some(session),
        };
        let invoker = {
            let registry = Arc::clone(&registry);
            thread::spawn(move || registry.execute(&installation, &execution))
        };
        let line = {
            let mut line = String::new();
            BufReader::new(client.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            line
        };
        let Some(ChannelFrame::Capability { id: invocation, .. }) =
            ChannelFrame::decode(line.trim())
        else {
            panic!("expected a capability frame");
        };

        // A file request bound to the invocation runs with the invoker's identity.
        client
            .write_all(
                format!(
                    "{}\n",
                    ChannelFrame::FileRequest {
                        id: "1".into(),
                        invocation: invocation.clone(),
                        action: "read".into(),
                        key: "0123456789abcdef0123456789abcdef".into(),
                        content_base64: None,
                    }
                    .encode()
                    .unwrap()
                )
                .as_bytes(),
            )
            .unwrap();
        client.flush().unwrap();
        let mut response = String::new();
        BufReader::new(client.try_clone().unwrap())
            .read_line(&mut response)
            .unwrap();
        let Some(ChannelFrame::FileResponse {
            ok,
            content_base64,
            size,
            ..
        }) = ChannelFrame::decode(response.trim())
        else {
            panic!("expected a file response");
        };
        assert!(ok);
        assert_eq!(size, Some(5));
        assert_eq!(content_base64.as_deref(), Some("aGVsbG8="));
        {
            let seen = files.seen.lock().unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(
                seen[0].0,
                ShellIdentity {
                    user_id: user,
                    session_id: session
                }
            );
            assert_eq!(seen[0].1, installation);
            assert_eq!(seen[0].2.key, "0123456789abcdef0123456789abcdef");
        }

        // An unknown invocation is refused before any executor runs.
        client
            .write_all(
                format!(
                    "{}\n",
                    ChannelFrame::FileRequest {
                        id: "2".into(),
                        invocation: "not-an-invocation".into(),
                        action: "read".into(),
                        key: "0123456789abcdef0123456789abcdef".into(),
                        content_base64: None,
                    }
                    .encode()
                    .unwrap()
                )
                .as_bytes(),
            )
            .unwrap();
        client.flush().unwrap();
        let mut denied = String::new();
        BufReader::new(client.try_clone().unwrap())
            .read_line(&mut denied)
            .unwrap();
        assert!(matches!(
            ChannelFrame::decode(denied.trim()),
            Some(ChannelFrame::FileResponse { ok: false, error: Some(error), .. }) if error == "no-invocation"
        ));
        assert_eq!(files.seen.lock().unwrap().len(), 1);

        // Finishing the invocation forgets its identity.
        client
            .write_all(
                format!(
                    "{}\n",
                    ChannelFrame::CapabilityResult {
                        id: invocation,
                        ok: true,
                        result: None,
                    }
                    .encode()
                    .unwrap()
                )
                .as_bytes(),
            )
            .unwrap();
        client.flush().unwrap();
        invoker.join().unwrap().unwrap();
        drop(client);
        handle.join().unwrap();
    }

    #[test]
    fn read_hello_authenticates_and_rejects_invalid_frames() {
        let installation = installation();
        let authenticator = FakeAuthenticator;

        let valid = ChannelFrame::Hello {
            protocol: CHANNEL_PROTOCOL.into(),
            installation_id: installation.to_string(),
            token: "valid-token".into(),
        };
        let mut reader = Cursor::new(format!("{}\n", valid.encode().unwrap()).into_bytes());
        assert_eq!(read_hello(&mut reader, &authenticator), Some(installation));

        let bad_token = ChannelFrame::Hello {
            protocol: CHANNEL_PROTOCOL.into(),
            installation_id: installation.to_string(),
            token: "nope".into(),
        };
        let mut reader = Cursor::new(format!("{}\n", bad_token.encode().unwrap()).into_bytes());
        assert_eq!(read_hello(&mut reader, &authenticator), None);

        let bad_protocol = ChannelFrame::Hello {
            protocol: "rumahl.channel.v0".into(),
            installation_id: installation.to_string(),
            token: "valid-token".into(),
        };
        let mut reader = Cursor::new(format!("{}\n", bad_protocol.encode().unwrap()).into_bytes());
        assert_eq!(read_hello(&mut reader, &authenticator), None);

        let mut reader = Cursor::new(b"garbage\n".to_vec());
        assert_eq!(read_hello(&mut reader, &authenticator), None);
    }

    #[test]
    fn bounded_reads_refuse_oversized_lines() {
        // A peer cannot grow a frame buffer without bound: the read stops one
        // byte past the limit and reports invalid data.
        let mut reader = Cursor::new(vec![b'x'; MAX_HELLO_BYTES + 10]);
        assert!(read_bounded_line(&mut reader, MAX_HELLO_BYTES).is_err());

        // A line that exactly fills the limit is still a valid frame.
        let mut reader = Cursor::new(format!("{}\n", "y".repeat(MAX_HELLO_BYTES - 1)).into_bytes());
        let line = read_bounded_line(&mut reader, MAX_HELLO_BYTES)
            .unwrap()
            .unwrap();
        assert_eq!(line.len(), MAX_HELLO_BYTES);

        let mut reader = Cursor::new(vec![b'{'; MAX_HELLO_BYTES + 10]);
        assert_eq!(read_hello(&mut reader, &FakeAuthenticator), None);
    }

    #[test]
    fn runs_a_connection_registers_and_unregisters() {
        use std::io::BufReader;
        use std::os::unix::net::UnixStream;
        use std::thread;
        use std::time::Duration;

        use rumahl_core::RuntimeChannelRegistry;

        let (server, mut client) = UnixStream::pair().unwrap();
        let reader = BufReader::new(server.try_clone().unwrap());
        let registry = Arc::new(RuntimeChannelRegistry::new());
        let authenticator: Arc<dyn ChannelAuthenticator> = Arc::new(FakeAuthenticator);
        let server_registry = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            run_connection(reader, server, server_registry, authenticator, None, None).unwrap()
        });

        let installation = installation();
        let hello = ChannelFrame::Hello {
            protocol: CHANNEL_PROTOCOL.into(),
            installation_id: installation.to_string(),
            token: "valid-token".into(),
        };
        client
            .write_all(format!("{}\n", hello.encode().unwrap()).as_bytes())
            .unwrap();
        client.flush().unwrap();

        let mut waited = 0;
        while !registry.is_registered(&installation) && waited < 200 {
            thread::sleep(Duration::from_millis(5));
            waited += 1;
        }
        assert!(registry.is_registered(&installation));

        registry
            .deliver(
                &installation,
                &RuntimeEvent {
                    topic: "rumahl.files.changed".into(),
                    resource: None,
                },
            )
            .unwrap();
        let mut events = BufReader::new(client.try_clone().unwrap());
        let mut line = String::new();
        events.read_line(&mut line).unwrap();
        assert!(line.contains("rumahl.files.changed"));

        // Closing every client handle unregisters the channel.
        drop(events);
        drop(client);
        handle.join().unwrap();
        assert!(!registry.is_registered(&installation));
    }

    #[test]
    fn token_store_issues_and_revokes() {
        let store = ChannelTokenStore::new();
        let id = installation();
        let token = store.issue(id);
        assert_eq!(
            ChannelAuthenticator::authenticate(&store, &id.to_string(), &token),
            Some(id)
        );
        assert_eq!(
            ChannelAuthenticator::authenticate(&store, &installation().to_string(), &token),
            None
        );
        assert_eq!(
            ChannelAuthenticator::authenticate(&store, &id.to_string(), "unknown"),
            None
        );
        store.revoke(&id);
        assert_eq!(
            ChannelAuthenticator::authenticate(&store, &id.to_string(), &token),
            None
        );
    }
}
