use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as UrlPath, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, ORIGIN};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rand::RngCore;
use rumahl_core::UserId;
use rumahl_ui_contracts::{ExtensionContribution, ShellEvent, ShellSnapshot};
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use url::Url;
use zeroize::Zeroizing;

use crate::backend::{ShellBackend, ShellBackendError, ShellIdentity};
use crate::events::ShellEventSource;
use crate::ssr;

const SESSION_COOKIE: &str = "__Host-rumahl_session";
const MAX_COOKIE_BYTES: usize = 4096;

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
}

pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route("/", get(shell))
        .route("/api/v1/shell/snapshot", get(snapshot))
        .route("/api/v1/shell/events", get(events))
        .route("/api/v1/shell/widgets/{id}/frame", get(widget_frame))
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
