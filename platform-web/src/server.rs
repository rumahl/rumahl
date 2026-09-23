use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, Path as UrlPath, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full, Limited};
use hyper::client::conn::http1;
use hyper::{Request, StatusCode as UpstreamStatus};
use hyper_util::rt::TokioIo;
use rand::RngCore;
use rumahl_core::UserId;
use rumahl_ui_contracts::{ExtensionContribution, ShellEvent, ShellSnapshot};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite;
use url::Url;
use zeroize::Zeroizing;

use crate::backend::{ShellBackend, ShellBackendError, ShellIdentity};
use crate::events::ShellEventSource;
use crate::ssr;
use crate::streams::{StreamAccess, StreamEndpoint, StreamProviderError, frame_path};

const SESSION_COOKIE: &str = "__Host-rumahl_session";
const MAX_COOKIE_BYTES: usize = 4096;
const STREAM_COOKIE: &str = "__Secure-rumahl_stream";
const STREAM_FRAME_CSP: &str = "default-src 'self'; script-src 'self' blob:; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' wss:; frame-ancestors 'self'; base-uri 'none'; object-src 'none'";

/// App routing owns this decision. The gateway verifies the resolved URL again
/// before exposing it to a browser or including its origin in the SSR CSP.
pub trait WidgetFrameResolver: Send + Sync + 'static {
    fn resolve(&self, user_id: UserId, contribution_id: &str, entrypoint: &str) -> Option<String>;
}

pub struct GatewayConfig {
    pub public_origin: Url,
    pub ssr_socket: PathBuf,
}

#[derive(Debug)]
pub enum GatewayError {
    InvalidOrigin,
    InvalidSocketPath,
    Io(std::io::Error),
}

impl GatewayConfig {
    pub fn new(public_origin: &str, ssr_socket: impl Into<PathBuf>) -> Result<Self, GatewayError> {
        let origin = Url::parse(public_origin).map_err(|_| GatewayError::InvalidOrigin)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || origin.username() != ""
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.origin().ascii_serialization() != public_origin
        {
            return Err(GatewayError::InvalidOrigin);
        }
        let ssr_socket = ssr_socket.into();
        if !ssr_socket.is_absolute() {
            return Err(GatewayError::InvalidSocketPath);
        }
        Ok(Self {
            public_origin: origin,
            ssr_socket,
        })
    }
}

pub struct GatewayState {
    pub config: GatewayConfig,
    pub backend: Arc<dyn ShellBackend>,
    pub events: Arc<dyn ShellEventSource>,
    pub widgets: Arc<dyn WidgetFrameResolver>,
    /// None keeps core Shell and recovery paths available without an engine.
    pub streams: Option<Arc<StreamAccess>>,
}

pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route("/", get(shell))
        .route("/api/v1/shell/snapshot", get(snapshot))
        .route("/api/v1/shell/events", get(events))
        .route("/api/v1/shell/widgets/{id}/frame", get(widget_frame))
        .route("/api/v1/shell/streams", get(stream_list))
        .route("/api/v1/shell/streams/{id}/grant", post(stream_grant))
        .route(
            "/api/v1/shell/streams/{id}/api/websockets",
            get(stream_socket),
        )
        .route("/api/v1/shell/streams/{id}/", get(stream_root))
        .route("/api/v1/shell/streams/{id}/{*tail}", get(stream_asset))
        .route("/recovery", get(recovery))
        .with_state(Arc::new(state))
}

/// Expose the router only on a private local socket. A separate HTTPS edge must
/// preserve Host, Origin and Cookie; it must not add trusted identity headers.
pub async fn serve(socket: &Path, state: GatewayState) -> Result<(), GatewayError> {
    if !socket.is_absolute() {
        return Err(GatewayError::InvalidSocketPath);
    }
    let parent = socket.parent().ok_or(GatewayError::InvalidSocketPath)?;
    let metadata = std::fs::symlink_metadata(parent).map_err(GatewayError::Io)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o027 != 0
    {
        return Err(GatewayError::InvalidSocketPath);
    }
    let listener = UnixListener::bind(socket).map_err(GatewayError::Io)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .map_err(GatewayError::Io)?;
    axum::serve(listener, router(state))
        .await
        .map_err(GatewayError::Io)
}

async fn shell(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(error) => return backend_error(error),
    };
    let snapshot = match load_snapshot(&state, &credential).await {
        Ok(snapshot) => snapshot,
        Err(error) => return backend_error(error),
    };
    let Ok(snapshot_json) = snapshot.to_json() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let frame_origins = frame_origins(&state, identity, &snapshot);
    let mut nonce_bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce: String = nonce_bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    match ssr::render(
        &state.config.ssr_socket,
        &snapshot_json,
        &nonce,
        &frame_origins,
    )
    .await
    {
        Ok(html) => {
            let mut response = Response::new(Body::from(html.body));
            response.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );
            if let Ok(csp) = HeaderValue::from_str(&html.csp) {
                response.headers_mut().insert(CONTENT_SECURITY_POLICY, csp);
            } else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            }
            secure_headers(&mut response);
            response
        }
        Err(()) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn snapshot(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    if let Err(error) = authenticate(&state, &credential).await {
        return backend_error(error);
    }
    let snapshot = match load_snapshot(&state, &credential).await {
        Ok(snapshot) => snapshot,
        Err(error) => return backend_error(error),
    };
    let Ok(json) = snapshot.to_json() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut response = Response::new(Body::from(json));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

async fn widget_frame(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(error) => return backend_error(error),
    };
    let snapshot = match load_snapshot(&state, &credential).await {
        Ok(snapshot) => snapshot,
        Err(error) => return backend_error(error),
    };
    let Some(entrypoint) = widget_entrypoint(&snapshot, &id) else {
        return error(StatusCode::NOT_FOUND);
    };
    let Some(frame_url) = state.widgets.resolve(identity.user_id, &id, entrypoint) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    if safe_frame_url(&state.config.public_origin, &frame_url).is_none() {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let descriptor =
        serde_json::json!({"frameVersion": 1, "contributionId": id, "frameUrl": frame_url});
    let mut response = Response::new(Body::from(descriptor.to_string()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

async fn stream_list(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(cause) => return backend_error(cause),
    };
    let Some(streams) = &state.streams else {
        return json_response(serde_json::json!({"sessions": []}));
    };
    let sessions = match streams.provider.list(identity) {
        Ok(sessions) => sessions,
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    json_response(serde_json::json!({
        "sessions": sessions.into_iter().map(|session| {
            serde_json::json!({"id": session.id, "title": session.title})
        }).collect::<Vec<_>>()
    }))
}

async fn stream_grant(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(cause) => return backend_error(cause),
    };
    let Some(streams) = &state.streams else {
        return error(StatusCode::NOT_FOUND);
    };
    let ticket = match streams.issue(identity, &id) {
        Ok(ticket) => ticket,
        Err(StreamProviderError::NotFound) => return error(StatusCode::NOT_FOUND),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    let mut response = json_response(serde_json::json!({"frameUrl": frame_path(&id)}));
    let cookie = format!(
        "{STREAM_COOKIE}={ticket}; Path={}; Max-Age=60; HttpOnly; Secure; SameSite=Strict",
        frame_path(&id)
    );
    match HeaderValue::from_str(&cookie) {
        Ok(value) => {
            response.headers_mut().insert(SET_COOKIE, value);
            response
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn authorized_stream(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
    id: &str,
) -> Result<(StreamEndpoint, Zeroizing<String>, ShellIdentity), Response> {
    let credential = credential(headers).map_err(|()| error(StatusCode::UNAUTHORIZED))?;
    let identity = authenticate(state, &credential)
        .await
        .map_err(backend_error)?;
    let ticket = stream_ticket(headers).ok_or_else(|| error(StatusCode::FORBIDDEN))?;
    let streams = state
        .streams
        .as_ref()
        .ok_or_else(|| error(StatusCode::NOT_FOUND))?;
    let endpoint = streams
        .authorize(identity, id, ticket)
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE))?
        .ok_or_else(|| error(StatusCode::FORBIDDEN))?;
    Ok((endpoint, credential, identity))
}

fn stream_ticket(headers: &HeaderMap) -> Option<&str> {
    let cookie = headers.get(COOKIE)?.to_str().ok()?;
    let mut found = None;
    for pair in cookie.split(';') {
        let (name, value) = pair.trim().split_once('=')?;
        if name == STREAM_COOKIE {
            if found.is_some()
                || value.len() != 64
                || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            found = Some(value);
        }
    }
    found
}

fn json_response(value: serde_json::Value) -> Response {
    let mut response = Response::new(Body::from(value.to_string()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

async fn stream_root(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    proxy_stream_asset(&state, &id, &uri, &headers).await
}

async fn stream_asset(
    State(state): State<Arc<GatewayState>>,
    UrlPath((id, _tail)): UrlPath<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    proxy_stream_asset(&state, &id, &uri, &headers).await
}

async fn proxy_stream_asset(
    state: &Arc<GatewayState>,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
) -> Response {
    let endpoint = match authorized_stream(state, headers, id).await {
        Ok((endpoint, _, _)) => endpoint,
        Err(response) => return response,
    };
    let prefix = frame_path(id);
    if !uri.path().starts_with(&prefix) || uri.to_string().len() > 2048 || uri.query().is_some() {
        return error(StatusCode::NOT_FOUND);
    }
    let tail = &uri.path()[prefix.len()..];
    if tail
        .split('/')
        .any(|segment| segment == ".." || segment.eq_ignore_ascii_case("%2e%2e"))
        || tail.starts_with("api/")
    {
        return error(StatusCode::NOT_FOUND);
    }
    let endpoint_uri = uri
        .path_and_query()
        .map_or_else(|| uri.path().to_owned(), ToString::to_string);
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let stream = UnixStream::connect(&endpoint.socket)
            .await
            .map_err(|_| ())?;
        let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| ())?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        let request = Request::builder()
            .method("GET")
            .uri(endpoint_uri)
            .header("host", "localhost")
            .header("authorization", format!("Bearer {}", endpoint.token()))
            .body(Full::new(hyper::body::Bytes::new()))
            .map_err(|_| ())?;
        let upstream = sender.send_request(request).await.map_err(|_| ())?;
        let status = upstream.status();
        let content_type = upstream.headers().get(CONTENT_TYPE).cloned();
        let body = Limited::new(upstream.into_body(), 16 * 1024 * 1024)
            .collect()
            .await
            .map_err(|_| ())?
            .to_bytes();
        Ok::<_, ()>((status, content_type, body))
    })
    .await;
    let Ok(Ok((status, content_type, body))) = result else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    if status != UpstreamStatus::OK && status != UpstreamStatus::NOT_FOUND {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(value) = content_type {
        response.headers_mut().insert(CONTENT_TYPE, value);
    }
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(STREAM_FRAME_CSP),
    );
    secure_headers(&mut response);
    response
}

async fn stream_socket(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let (endpoint, credential, identity) = match authorized_stream(&state, &headers, &id).await {
        Ok(values) => values,
        Err(response) => return response,
    };
    let upstream = match connect_stream_socket(&endpoint).await {
        Ok(socket) => socket,
        Err(()) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    upgrade
        .max_message_size(16 * 1024 * 1024)
        .max_frame_size(16 * 1024 * 1024)
        .on_upgrade(move |browser| {
            relay_stream(browser, upstream, state, credential, identity, endpoint)
        })
        .into_response()
}

async fn connect_stream_socket(
    endpoint: &StreamEndpoint,
) -> Result<tokio_tungstenite::WebSocketStream<UnixStream>, ()> {
    use tungstenite::client::IntoClientRequest;

    let url = format!(
        "ws://localhost{}api/websockets?token={}",
        frame_path(&endpoint.id),
        endpoint.token()
    );
    let mut request = url.into_client_request().map_err(|_| ())?;
    request
        .headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("http://localhost"));
    let stream = tokio::time::timeout(
        Duration::from_secs(5),
        UnixStream::connect(&endpoint.socket),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::client_async(request, stream),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    Ok(socket)
}

async fn relay_stream(
    mut browser: WebSocket,
    mut upstream: tokio_tungstenite::WebSocketStream<UnixStream>,
    state: Arc<GatewayState>,
    credential: Zeroizing<String>,
    identity: ShellIdentity,
    endpoint: StreamEndpoint,
) {
    let mut recheck = tokio::time::interval(Duration::from_secs(30));
    recheck.tick().await;
    loop {
        tokio::select! {
            _ = recheck.tick() => {
                let active = authenticate(&state, &credential).await == Ok(identity)
                    && state.streams.as_ref().is_some_and(|streams| {
                        streams.provider.resolve(identity, &endpoint.id).ok().flatten()
                            .is_some_and(|current| current.socket == endpoint.socket
                                && current.token() == endpoint.token())
                    });
                if !active { break; }
            }
            message = browser.recv() => {
                let Some(Ok(message)) = message else { break; };
                let upstream_message = match message {
                    Message::Text(text) => tungstenite::Message::Text(text.to_string().into()),
                    Message::Binary(bytes) => tungstenite::Message::Binary(bytes),
                    Message::Ping(bytes) => tungstenite::Message::Ping(bytes),
                    Message::Pong(bytes) => tungstenite::Message::Pong(bytes),
                    Message::Close(_) => break,
                };
                if upstream.send(upstream_message).await.is_err() { break; }
            }
            message = upstream.next() => {
                let Some(Ok(message)) = message else { break; };
                let browser_message = match message {
                    tungstenite::Message::Text(text) => Message::Text(text.to_string().into()),
                    tungstenite::Message::Binary(bytes) => Message::Binary(bytes),
                    tungstenite::Message::Ping(bytes) => Message::Ping(bytes),
                    tungstenite::Message::Pong(bytes) => Message::Pong(bytes),
                    tungstenite::Message::Close(_) => break,
                    tungstenite::Message::Frame(_) => continue,
                };
                if browser.send(browser_message).await.is_err() { break; }
            }
        }
    }
    let _ = upstream.send(tungstenite::Message::Close(None)).await;
    let _ = browser.send(Message::Close(None)).await;
}

async fn events(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let Ok(credential) = credential(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(error) => return backend_error(error),
    };
    let subscriber = state.events.subscribe();
    upgrade
        .max_message_size(1024)
        .max_frame_size(1024)
        .on_upgrade(move |socket| drive_events(socket, state, credential, identity, subscriber))
        .into_response()
}

async fn drive_events(
    mut socket: WebSocket,
    state: Arc<GatewayState>,
    credential: Zeroizing<String>,
    identity: ShellIdentity,
    mut subscriber: broadcast::Receiver<(UserId, ShellEvent)>,
) {
    let mut recheck = tokio::time::interval(Duration::from_secs(30));
    recheck.tick().await;
    loop {
        tokio::select! {
            _ = recheck.tick() => {
                if authenticate(&state, &credential).await != Ok(identity) { break; }
            }
            next = subscriber.recv() => match next {
                Ok((user_id, event)) if user_id == identity.user_id => {
                    if authenticate(&state, &credential).await != Ok(identity) { break; }
                    let revoke = matches!(event, ShellEvent::SessionRevoked { .. });
                    if socket.send(Message::Text(event.to_json().into())).await.is_err() { break; }
                    if revoke { break; }
                }
                Ok(_) => {},
                Err(broadcast::error::RecvError::Lagged(_)) | Err(broadcast::error::RecvError::Closed) => break,
            },
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Ping(payload)))
                        if socket.send(Message::Pong(payload.clone())).await.is_err() => break,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {},
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

async fn recovery(headers: HeaderMap) -> Response {
    let german = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("de"));
    let body = if german {
        "<!doctype html><html lang=\"de\"><meta charset=\"utf-8\"><title>rumahl OS Wiederherstellung</title><h1>Wiederherstellung</h1><p>Diese Seite funktioniert ohne App-Container und Shell-Renderer. Der sichere Wiederherstellungsdienst ist noch nicht eingerichtet.</p><p><a href=\"/\">Shell erneut laden</a></p></html>"
    } else {
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>rumahl OS Recovery</title><h1>Recovery</h1><p>This page works without app containers or the shell renderer. A privileged recovery service is not configured yet.</p><p><a href=\"/\">Reload shell</a></p></html>"
    };
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    secure_headers(&mut response);
    response
}

fn credential(headers: &HeaderMap) -> Result<Zeroizing<String>, ()> {
    if headers.get_all(COOKIE).iter().count() != 1 {
        return Err(());
    }
    let value = headers.get(COOKIE).ok_or(())?.to_str().map_err(|_| ())?;
    if value.len() > MAX_COOKIE_BYTES {
        return Err(());
    }
    let mut found = None;
    for pair in value.split(';') {
        let Some((name, token)) = pair.trim().split_once('=') else {
            return Err(());
        };
        if name == SESSION_COOKIE {
            if found.is_some()
                || token.len() != 43
                || !token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            {
                return Err(());
            }
            found = Some(Zeroizing::new(token.to_owned()));
        }
    }
    found.ok_or(())
}

fn valid_origin(headers: &HeaderMap, expected: &Url) -> bool {
    headers.get_all(ORIGIN).iter().count() == 1
        && headers.get(ORIGIN).and_then(|value| value.to_str().ok())
            == Some(expected.origin().ascii_serialization().as_str())
}

async fn authenticate(
    state: &Arc<GatewayState>,
    credential: &str,
) -> Result<ShellIdentity, ShellBackendError> {
    let backend = Arc::clone(&state.backend);
    let credential = Zeroizing::new(credential.to_owned());
    tokio::task::spawn_blocking(move || backend.authenticate(&credential))
        .await
        .map_err(|_| ShellBackendError::Unavailable)?
}

async fn load_snapshot(
    state: &Arc<GatewayState>,
    credential: &str,
) -> Result<ShellSnapshot, ShellBackendError> {
    let backend = Arc::clone(&state.backend);
    let credential = Zeroizing::new(credential.to_owned());
    tokio::task::spawn_blocking(move || backend.snapshot(&credential))
        .await
        .map_err(|_| ShellBackendError::Unavailable)?
}

fn widget_entrypoint<'a>(snapshot: &'a ShellSnapshot, id: &str) -> Option<&'a str> {
    snapshot
        .contributions()
        .iter()
        .find_map(|contribution| match contribution {
            ExtensionContribution::Widget {
                id: candidate,
                entrypoint,
                ..
            } if candidate == id => Some(entrypoint.as_str()),
            _ => None,
        })
}

fn safe_frame_url(shell_origin: &Url, value: &str) -> Option<String> {
    if value.len() > 2048 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str()? == shell_origin.host_str()?
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn frame_origins(
    state: &GatewayState,
    identity: ShellIdentity,
    snapshot: &ShellSnapshot,
) -> Vec<String> {
    let mut origins = BTreeSet::new();
    for contribution in snapshot.contributions() {
        if let ExtensionContribution::Widget { id, entrypoint, .. } = contribution
            && let Some(url) = state.widgets.resolve(identity.user_id, id, entrypoint)
            && let Some(origin) = safe_frame_url(&state.config.public_origin, &url)
        {
            origins.insert(origin);
        }
    }
    origins.into_iter().take(32).collect()
}

fn secure_headers(response: &mut Response) {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
}

fn error(status: StatusCode) -> Response {
    let mut response = Response::new(Body::from(if status == StatusCode::SERVICE_UNAVAILABLE {
        "Shell unavailable. Open /recovery."
    } else {
        "Request unavailable."
    }));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

fn backend_error(cause: ShellBackendError) -> Response {
    error(match cause {
        ShellBackendError::Unauthorized => StatusCode::UNAUTHORIZED,
        ShellBackendError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use rumahl_core::SessionId;
    use rumahl_ui_contracts::{
        ShellSystemStatus, ShellTheme, ShellUser, SystemProtectionStatus, WindowChromeVariant,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;

    const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    struct TestBackend {
        identity: ShellIdentity,
        available: bool,
    }

    impl ShellBackend for TestBackend {
        fn authenticate(&self, token: &str) -> Result<ShellIdentity, ShellBackendError> {
            if token == TOKEN {
                Ok(self.identity)
            } else {
                Err(ShellBackendError::Unauthorized)
            }
        }

        fn snapshot(&self, token: &str) -> Result<ShellSnapshot, ShellBackendError> {
            if !self.available || token != TOKEN {
                return Err(ShellBackendError::Unavailable);
            }
            Ok(ShellSnapshot::new(
                "shell-build-001", "revision-001",
                ShellUser::new("Example User", "en-US").unwrap(),
                ShellTheme::new(
                    "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
                    WindowChromeVariant::Standard,
                ).unwrap(),
                ShellSystemStatus::new(SystemProtectionStatus::Active, 1, 1, None).unwrap(),
                vec![ExtensionContribution::widget("com.rumahl.weather.widget", "Weather", "main").unwrap()],
            ).unwrap())
        }
    }

    struct TestWidgets(&'static str);

    impl WidgetFrameResolver for TestWidgets {
        fn resolve(&self, _: UserId, _: &str, _: &str) -> Option<String> {
            Some(self.0.to_owned())
        }
    }

    fn state(available: bool, widget_url: &'static str) -> GatewayState {
        GatewayState {
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestBackend {
                identity: ShellIdentity {
                    user_id: UserId::new(),
                    session_id: SessionId::new(),
                },
                available,
            }),
            events: Arc::new(crate::InMemoryShellEvents::new(4)),
            widgets: Arc::new(TestWidgets(widget_url)),
            streams: None,
        }
    }

    fn request(path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn snapshot_requires_real_cookie_and_ignores_identity_headers() {
        let app = router(state(true, "https://weather.apps.rumahl.dev/widget"));
        let accepted = app
            .clone()
            .oneshot(request("/api/v1/shell/snapshot"))
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);
        assert_eq!(accepted.headers()[CACHE_CONTROL], "private, no-store");

        let spoofed = Request::builder()
            .uri("/api/v1/shell/snapshot")
            .header("x-rumahl-user-id", UserId::new().to_string())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(spoofed).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );

        let duplicate = Request::builder()
            .uri("/api/v1/shell/snapshot")
            .header(
                COOKIE,
                format!("{SESSION_COOKIE}={TOKEN}; {SESSION_COOKIE}={TOKEN}"),
            )
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(duplicate).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    async fn read_http(stream: &mut UnixStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let read = stream.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&request[..end]);
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    return String::from_utf8(request).unwrap();
                }
            }
        }
    }

    #[tokio::test]
    async fn stream_grants_and_proxy_keep_engine_credentials_server_side() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("engine.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let id = "4485f47e-a1cd-4b7b-a7c2-203086be13f5";
        let identity = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let endpoint = StreamEndpoint::new(id, "Firefox", identity, &socket).unwrap();
        let engine_token = endpoint.token().to_owned();
        let engine = tokio::spawn(async move {
            let (mut provisioning, _) = listener.accept().await.unwrap();
            let request = read_http(&mut provisioning).await;
            assert!(request.starts_with(&format!(
                "POST /api/v1/shell/streams/{id}/api/tokens HTTP/1.1"
            )));
            assert!(request.contains("authorization: Bearer "));
            assert!(request.contains(&engine_token));
            provisioning
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nOK")
                .await
                .unwrap();
            let (mut asset, _) = listener.accept().await.unwrap();
            let request = read_http(&mut asset).await;
            assert!(request.starts_with(&format!("GET /api/v1/shell/streams/{id}/ HTTP/1.1")));
            assert!(request.contains(&format!("authorization: Bearer {engine_token}")));
            assert!(!request.to_ascii_lowercase().contains("cookie:"));
            let body = "<html>stream core</html>";
            asset.write_all(format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            ).as_bytes()).await.unwrap();
        });
        let provider = Arc::new(crate::RegisteredStreamProvider::new());
        provider
            .register_selkies(endpoint, &"M".repeat(48))
            .await
            .unwrap();
        let app = router(GatewayState {
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestBackend {
                identity,
                available: true,
            }),
            events: Arc::new(crate::InMemoryShellEvents::new(4)),
            widgets: Arc::new(TestWidgets("https://weather.apps.rumahl.dev/widget")),
            streams: Some(Arc::new(StreamAccess::new(provider))),
        });
        let list = app
            .clone()
            .oneshot(request("/api/v1/shell/streams"))
            .await
            .unwrap();
        assert_eq!(list.status(), StatusCode::OK);
        let body = list.into_body().collect().await.unwrap().to_bytes();
        assert!(std::str::from_utf8(&body).unwrap().contains("Firefox"));
        let rejected = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/shell/streams/{id}/grant"))
            .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(rejected).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let grant = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/shell/streams/{id}/grant"))
            .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"))
            .header(ORIGIN, "https://rumahl.dev")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(grant).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[SET_COOKIE].to_str().unwrap().to_owned();
        assert!(cookie.contains("HttpOnly; Secure; SameSite=Strict"));
        assert!(!cookie.contains(&"M".repeat(48)));
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            String::from_utf8(body.to_vec()).unwrap(),
            format!("{{\"frameUrl\":\"/api/v1/shell/streams/{id}/\"}}")
        );
        let ticket = cookie.split(';').next().unwrap();
        let frame = Request::builder()
            .uri(format!("/api/v1/shell/streams/{id}/"))
            .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}; {ticket}"))
            .body(Body::empty())
            .unwrap();
        let frame = app.clone().oneshot(frame).await.unwrap();
        assert_eq!(frame.status(), StatusCode::OK);
        assert_eq!(frame.headers()[CONTENT_SECURITY_POLICY], STREAM_FRAME_CSP);
        let body = frame.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"<html>stream core</html>");
        let still_alive = app
            .oneshot(request("/api/v1/shell/snapshot"))
            .await
            .unwrap();
        assert_eq!(still_alive.status(), StatusCode::OK);
        engine.await.unwrap();
    }

    #[tokio::test]
    #[expect(
        clippy::result_large_err,
        reason = "tokio-tungstenite handshake callback uses a large response error"
    )]
    async fn websocket_relay_uses_server_only_token_over_unix_sockets() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let directory = tempfile::tempdir().unwrap();
        let engine_socket = directory.path().join("engine.sock");
        let engine_listener = UnixListener::bind(&engine_socket).unwrap();
        let id = "4d5a0ec1-6846-44c9-a9c9-b61315119cdf";
        let identity = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let endpoint = StreamEndpoint::new(id, "Browser", identity, &engine_socket).unwrap();
        let engine_token = endpoint.token().to_owned();
        let engine = tokio::spawn(async move {
            let (mut provisioning, _) = engine_listener.accept().await.unwrap();
            let _ = read_http(&mut provisioning).await;
            provisioning
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nOK")
                .await
                .unwrap();
            let (socket, _) = engine_listener.accept().await.unwrap();
            let mut upstream = tokio_tungstenite::accept_hdr_async(
                socket,
                move |request: &tungstenite::handshake::server::Request,
                      response: tungstenite::handshake::server::Response| {
                    assert_eq!(
                        request.uri().path(),
                        format!("/api/v1/shell/streams/{id}/api/websockets")
                    );
                    assert_eq!(
                        request.uri().query(),
                        Some(format!("token={engine_token}").as_str())
                    );
                    assert_eq!(request.headers()[ORIGIN], "http://localhost");
                    Ok(response)
                },
            )
            .await
            .unwrap();
            upstream
                .send(tungstenite::Message::Text("ready".into()))
                .await
                .unwrap();
            let message = upstream.next().await.unwrap().unwrap();
            assert_eq!(message.into_text().unwrap(), "hello");
            upstream
                .send(tungstenite::Message::Text("echo".into()))
                .await
                .unwrap();
        });
        let provider = Arc::new(crate::RegisteredStreamProvider::new());
        provider
            .register_selkies(endpoint, &"M".repeat(48))
            .await
            .unwrap();
        let app = router(GatewayState {
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestBackend {
                identity,
                available: true,
            }),
            events: Arc::new(crate::InMemoryShellEvents::new(4)),
            widgets: Arc::new(TestWidgets("https://weather.apps.rumahl.dev/widget")),
            streams: Some(Arc::new(StreamAccess::new(provider))),
        });
        let grant = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/shell/streams/{id}/grant"))
            .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"))
            .header(ORIGIN, "https://rumahl.dev")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(grant).await.unwrap();
        let ticket = response.headers()[SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let gateway_socket = directory.path().join("gateway.sock");
        let listener = UnixListener::bind(&gateway_socket).unwrap();
        let gateway = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let browser_stream = UnixStream::connect(&gateway_socket).await.unwrap();
        let mut request = format!("ws://localhost/api/v1/shell/streams/{id}/api/websockets")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_static("https://rumahl.dev"));
        request.headers_mut().insert(
            COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={TOKEN}; {ticket}")).unwrap(),
        );
        assert!(request.uri().query().is_none());
        let (mut browser, _) = tokio_tungstenite::client_async(request, browser_stream)
            .await
            .unwrap();
        assert_eq!(
            browser.next().await.unwrap().unwrap().into_text().unwrap(),
            "ready"
        );
        browser
            .send(tungstenite::Message::Text("hello".into()))
            .await
            .unwrap();
        assert_eq!(
            browser.next().await.unwrap().unwrap().into_text().unwrap(),
            "echo"
        );
        gateway.abort();
        engine.await.unwrap();
    }

    #[tokio::test]
    async fn recovery_stays_available_when_snapshot_and_renderer_are_unavailable() {
        let app = router(state(false, "https://weather.apps.rumahl.dev/widget"));
        assert_eq!(
            app.clone().oneshot(request("/")).await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/recovery")
                    .header("accept-language", "de-DE,de;q=0.9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("lang=\"de\""));
    }

    #[tokio::test]
    async fn widget_must_be_authorized_and_on_isolated_https_host() {
        let app = router(state(true, "https://weather.apps.rumahl.dev/widget"));
        let response = app
            .clone()
            .oneshot(request(
                "/api/v1/shell/widgets/com.rumahl.weather.widget/frame",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("weather.apps.rumahl.dev")
        );
        assert_eq!(
            app.clone()
                .oneshot(request("/api/v1/shell/widgets/com.rumahl.hidden/frame"))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let unsafe_app = router(state(true, "https://rumahl.dev/widget"));
        assert_eq!(
            unsafe_app
                .oneshot(request(
                    "/api/v1/shell/widgets/com.rumahl.weather.widget/frame"
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn websocket_origin_must_match_exact_public_origin() {
        let expected = Url::parse("https://rumahl.dev").unwrap();
        let mut headers = HeaderMap::new();
        assert!(!valid_origin(&headers, &expected));
        headers.insert(ORIGIN, HeaderValue::from_static("https://attacker.example"));
        assert!(!valid_origin(&headers, &expected));
        headers.insert(ORIGIN, HeaderValue::from_static("https://rumahl.dev"));
        assert!(valid_origin(&headers, &expected));
        headers.append(ORIGIN, HeaderValue::from_static("https://rumahl.dev"));
        assert!(!valid_origin(&headers, &expected));
    }

    #[test]
    fn rejects_insecure_configuration_and_frame_urls() {
        assert!(GatewayConfig::new("http://rumahl.dev", "/run/ssr.sock").is_err());
        assert!(GatewayConfig::new("https://rumahl.dev/path", "/run/ssr.sock").is_err());
        assert!(GatewayConfig::new("https://rumahl.dev", "relative.sock").is_err());
        let shell = Url::parse("https://rumahl.dev").unwrap();
        for url in [
            "http://weather.apps.rumahl.dev/widget",
            "https://rumahl.dev/widget",
            "https://user:pass@weather.apps.rumahl.dev/widget",
            "https://weather.apps.rumahl.dev/widget?token=secret",
        ] {
            assert!(safe_frame_url(&shell, url).is_none());
        }
    }
}
