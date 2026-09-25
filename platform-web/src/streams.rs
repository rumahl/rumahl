//! Optional streamed-app sessions. The Shell never depends on this provider
//! for its own snapshot, events, or recovery paths.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use rand::RngCore;
use rumahl_core::{SessionId, UserId};
use tokio::net::UnixStream;
use zeroize::Zeroizing;

use crate::backend::ShellIdentity;

const GRANT_LIFETIME: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamDescriptor {
    pub id: String,
    pub title: String,
}

#[derive(Clone)]
pub struct StreamEndpoint {
    pub id: String,
    pub title: String,
    pub owner: ShellIdentity,
    pub socket: PathBuf,
    token: Arc<Zeroizing<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEndpointError {
    InvalidMetadata,
}

impl StreamEndpoint {
    pub fn new(
        id: &str,
        title: &str,
        owner: ShellIdentity,
        socket: impl Into<PathBuf>,
    ) -> Result<Self, StreamEndpointError> {
        if uuid::Uuid::try_parse(id)
            .ok()
            .is_none_or(|parsed| parsed.to_string() != id)
            || title.is_empty()
            || title.len() > 128
            || title.chars().any(char::is_control)
        {
            return Err(StreamEndpointError::InvalidMetadata);
        }
        let socket = socket.into();
        if !socket.is_absolute()
            || socket
                .components()
                .any(|part| part == std::path::Component::ParentDir)
        {
            return Err(StreamEndpointError::InvalidMetadata);
        }
        let mut random = [0_u8; 32];
        rand::rng().fill_bytes(&mut random);
        Ok(Self {
            id: id.to_owned(),
            title: title.to_owned(),
            owner,
            socket,
            token: Arc::new(Zeroizing::new(hex_token(&random))),
        })
    }

    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamProviderError {
    Unavailable,
    AlreadyRegistered,
    NotFound,
}

pub trait StreamProvider: Send + Sync + 'static {
    fn list(&self, identity: ShellIdentity) -> Result<Vec<StreamDescriptor>, StreamProviderError>;
    fn resolve(
        &self,
        identity: ShellIdentity,
        id: &str,
    ) -> Result<Option<StreamEndpoint>, StreamProviderError>;
}

/// A runtime adapter registers only already-started, isolated sessions.
/// Registrations are per login session, so another user or later login cannot
/// inherit an engine socket or a server-side access credential.
pub struct RegisteredStreamProvider {
    sessions: RwLock<HashMap<String, StreamEndpoint>>,
    registration: tokio::sync::Mutex<()>,
}

impl RegisteredStreamProvider {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            registration: tokio::sync::Mutex::new(()),
        }
    }

    /// Provision a server-only Selkies token before publishing the session.
    /// An alternative streaming engine can implement StreamProvider directly.
    pub async fn register_selkies(
        &self,
        endpoint: StreamEndpoint,
        master_token: &str,
    ) -> Result<(), StreamProviderError> {
        let _registration = self.registration.lock().await;
        if self
            .sessions
            .read()
            .map_err(|_| StreamProviderError::Unavailable)?
            .contains_key(&endpoint.id)
        {
            return Err(StreamProviderError::AlreadyRegistered);
        }
        provision_selkies_token(&endpoint, master_token).await?;
        self.sessions
            .write()
            .map_err(|_| StreamProviderError::Unavailable)?
            .insert(endpoint.id.clone(), endpoint);
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.remove(id);
        }
    }
}

impl Default for RegisteredStreamProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamProvider for RegisteredStreamProvider {
    fn list(&self, identity: ShellIdentity) -> Result<Vec<StreamDescriptor>, StreamProviderError> {
        let mut result: Vec<_> = self
            .sessions
            .read()
            .map_err(|_| StreamProviderError::Unavailable)?
            .values()
            .filter(|session| session.owner == identity)
            .map(|session| StreamDescriptor {
                id: session.id.clone(),
                title: session.title.clone(),
            })
            .collect();
        result.sort_by(|a, b| a.title.cmp(&b.title).then(a.id.cmp(&b.id)));
        Ok(result)
    }

    fn resolve(
        &self,
        identity: ShellIdentity,
        id: &str,
    ) -> Result<Option<StreamEndpoint>, StreamProviderError> {
        Ok(self
            .sessions
            .read()
            .map_err(|_| StreamProviderError::Unavailable)?
            .get(id)
            .filter(|session| session.owner == identity)
            .cloned())
    }
}

pub struct StreamAccess {
    pub provider: Arc<dyn StreamProvider>,
    grants: Mutex<HashMap<(UserId, SessionId, String), BrowserGrant>>,
}

struct BrowserGrant {
    ticket: Zeroizing<String>,
    endpoint_token: Zeroizing<String>,
    expires: Instant,
}

impl StreamAccess {
    pub fn new(provider: Arc<dyn StreamProvider>) -> Self {
        Self {
            provider,
            grants: Mutex::new(HashMap::new()),
        }
    }

    pub fn issue(&self, identity: ShellIdentity, id: &str) -> Result<String, StreamProviderError> {
        let endpoint = self
            .provider
            .resolve(identity, id)?
            .ok_or(StreamProviderError::NotFound)?;
        let mut random = [0_u8; 32];
        rand::rng().fill_bytes(&mut random);
        let ticket = hex_token(&random);
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| StreamProviderError::Unavailable)?;
        grants.retain(|_, value| value.expires > Instant::now());
        grants.insert(
            (identity.user_id, identity.session_id, id.to_owned()),
            BrowserGrant {
                ticket: Zeroizing::new(ticket.clone()),
                endpoint_token: Zeroizing::new(endpoint.token().to_owned()),
                expires: Instant::now() + GRANT_LIFETIME,
            },
        );
        Ok(ticket)
    }

    pub fn authorize(
        &self,
        identity: ShellIdentity,
        id: &str,
        ticket: &str,
    ) -> Result<Option<StreamEndpoint>, StreamProviderError> {
        let endpoint = self.provider.resolve(identity, id)?;
        let grants = self
            .grants
            .lock()
            .map_err(|_| StreamProviderError::Unavailable)?;
        let allowed = endpoint.as_ref().is_some_and(|current| {
            grants
                .get(&(identity.user_id, identity.session_id, id.to_owned()))
                .is_some_and(|grant| {
                    grant.expires > Instant::now()
                        && grant.ticket.as_str() == ticket
                        && grant.endpoint_token.as_str() == current.token()
                })
        });
        Ok(endpoint.filter(|_| allowed))
    }
}

async fn provision_selkies_token(
    endpoint: &StreamEndpoint,
    master_token: &str,
) -> Result<(), StreamProviderError> {
    if master_token.len() < 32 || master_token.bytes().any(|byte| !byte.is_ascii_graphic()) {
        return Err(StreamProviderError::Unavailable);
    }
    let body = serde_json::json!({endpoint.token(): {"role": "controller", "slot": 1}});
    let encoded = serde_json::to_vec(&body).map_err(|_| StreamProviderError::Unavailable)?;
    let path = format!("/api/v1/shell/streams/{}/api/tokens", endpoint.id);
    let socket = endpoint.socket.clone();
    tokio::time::timeout(Duration::from_secs(5), async move {
        let stream = UnixStream::connect(socket)
            .await
            .map_err(|_| StreamProviderError::Unavailable)?;
        let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| StreamProviderError::Unavailable)?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header("host", "localhost")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {master_token}"))
            .body(Full::new(Bytes::from(encoded)))
            .map_err(|_| StreamProviderError::Unavailable)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| StreamProviderError::Unavailable)?;
        if response.status() != StatusCode::OK {
            return Err(StreamProviderError::Unavailable);
        }
        Limited::new(response.into_body(), 1024)
            .collect()
            .await
            .map_err(|_| StreamProviderError::Unavailable)?;
        Ok(())
    })
    .await
    .map_err(|_| StreamProviderError::Unavailable)?
}

pub fn frame_path(id: &str) -> String {
    format!("/api/v1/shell/streams/{id}/")
}

fn hex_token(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ReplaceableProvider(RwLock<StreamEndpoint>);

    impl StreamProvider for ReplaceableProvider {
        fn list(&self, _: ShellIdentity) -> Result<Vec<StreamDescriptor>, StreamProviderError> {
            Ok(Vec::new())
        }

        fn resolve(
            &self,
            identity: ShellIdentity,
            id: &str,
        ) -> Result<Option<StreamEndpoint>, StreamProviderError> {
            let endpoint = self
                .0
                .read()
                .map_err(|_| StreamProviderError::Unavailable)?;
            Ok((endpoint.owner == identity && endpoint.id == id).then(|| endpoint.clone()))
        }
    }

    #[test]
    fn grants_are_bound_to_user_login_and_engine_generation() {
        let owner = ShellIdentity {
            user_id: rumahl_core::UserId::new(),
            session_id: SessionId::new(),
        };
        let other_login = ShellIdentity {
            user_id: owner.user_id,
            session_id: SessionId::new(),
        };
        let id = "4485f47e-a1cd-4b7b-a7c2-203086be13f5";
        let socket = "/run/rumahl/streaming/app.sock";
        let endpoint = StreamEndpoint::new(id, "Firefox", owner, socket).unwrap();
        let provider = Arc::new(ReplaceableProvider(RwLock::new(endpoint)));
        let access = StreamAccess::new(provider.clone());
        let ticket = access.issue(owner, id).unwrap();
        assert!(access.authorize(owner, id, &ticket).unwrap().is_some());
        assert!(
            access
                .authorize(other_login, id, &ticket)
                .unwrap()
                .is_none()
        );
        *provider.0.write().unwrap() = StreamEndpoint::new(id, "Firefox", owner, socket).unwrap();
        assert!(access.authorize(owner, id, &ticket).unwrap().is_none());
    }

    #[test]
    fn endpoint_rejects_noncanonical_id_and_relative_socket() {
        let owner = ShellIdentity {
            user_id: rumahl_core::UserId::new(),
            session_id: SessionId::new(),
        };
        assert!(StreamEndpoint::new("not-a-uuid", "Firefox", owner, "/run/app.sock").is_err());
        assert!(
            StreamEndpoint::new(
                "4485f47e-a1cd-4b7b-a7c2-203086be13f5",
                "Firefox",
                owner,
                "app.sock"
            )
            .is_err()
        );
    }
}
