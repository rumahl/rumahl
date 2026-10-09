//! Persistent app <-> OS channel transport.
//!
//! A running app instance (typically an always-on service) connects to a
//! loopback endpoint and authenticates with a per-installation token. The OS
//! then registers a live [`RuntimeChannel`] for it, so OS events reach the app
//! even while it has no window open. Frames are JSON lines, versioned by
//! `rumahl.channel.v1`.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use rand::RngCore;
use rumahl_core::{
    InstallationId, RuntimeChannel, RuntimeChannelError, RuntimeChannelRegistry, RuntimeEvent,
};
use serde::{Deserialize, Serialize};

pub const CHANNEL_PROTOCOL: &str = "rumahl.channel.v1";

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

/// Writes OS events to one app connection as JSON lines.
pub struct SocketRuntimeChannel<W: Write + Send> {
    writer: Mutex<W>,
}

impl<W: Write + Send> SocketRuntimeChannel<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send> RuntimeChannel for SocketRuntimeChannel<W> {
    fn deliver(&self, event: &RuntimeEvent) -> Result<(), RuntimeChannelError> {
        let frame = ChannelFrame::Event {
            topic: event.topic.clone(),
            resource: event.resource.clone(),
        };
        let line = frame.encode().map_err(|_| RuntimeChannelError::Rejected)?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| RuntimeChannelError::Unavailable)?;
        writeln!(writer, "{line}").map_err(|_| RuntimeChannelError::Unavailable)?;
        writer.flush().map_err(|_| RuntimeChannelError::Unavailable)
    }
}

/// Reads the first frame of a connection and returns the authenticated
/// installation. Returns `None` for a missing/invalid `hello`.
pub fn read_hello(
    reader: &mut impl BufRead,
    authenticator: &dyn ChannelAuthenticator,
) -> Option<InstallationId> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
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
pub fn run_connection<R, W>(
    mut reader: R,
    writer: W,
    registry: Arc<RuntimeChannelRegistry>,
    authenticator: Arc<dyn ChannelAuthenticator>,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write + Send + 'static,
{
    let Some(installation) = read_hello(&mut reader, authenticator.as_ref()) else {
        return Ok(());
    };
    let channel = Arc::new(SocketRuntimeChannel::new(writer));
    if !registry.register(installation, channel) {
        return Ok(());
    }
    // Drain frames until the app disconnects (0 bytes) or the socket errors.
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
    registry.unregister(&installation);
    Ok(())
}

/// Accepts app channel connections forever, spawning a thread per connection.
/// Runs until the listener errors (for example when the socket is removed).
pub fn serve_channel_listener(
    listener: UnixListener,
    registry: Arc<RuntimeChannelRegistry>,
    authenticator: Arc<dyn ChannelAuthenticator>,
) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Ok(writer) = stream.try_clone() else { continue };
        let reader = BufReader::new(stream);
        let registry = Arc::clone(&registry);
        let authenticator = Arc::clone(&authenticator);
        std::thread::spawn(move || {
            let _ = run_connection(reader, writer, registry, authenticator);
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
        assert_eq!(written, "{\"kind\":\"event\",\"topic\":\"rumahl.apps.installed\"}\n");
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
        let handle =
            thread::spawn(move || run_connection(reader, server, server_registry, authenticator).unwrap());

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
