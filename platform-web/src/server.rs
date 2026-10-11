use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, Path as UrlPath, Query, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full, Limited};
use hyper::client::conn::http1;
use hyper::{Request, StatusCode as UpstreamStatus};
use hyper_util::rt::TokioIo;
use rand::RngCore;
use rumahl_core::{BrowserProfileId, UserId};
use rumahl_ui_contracts::{ExtensionContribution, ShellEvent, ShellSnapshot};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite;
use url::Url;
use zeroize::Zeroizing;

use crate::backend::{ShellBackend, ShellBackendError, ShellIdentity};
use crate::events::ShellEventSource;
use crate::oidc;
use crate::ssr;
use crate::streams::{StreamAccess, StreamEndpoint, StreamProviderError, frame_path};

const SESSION_COOKIE: &str = "__Host-rumahl_session";
const MAX_COOKIE_BYTES: usize = 4096;
const STREAM_COOKIE: &str = "__Secure-rumahl_stream";
/// Non-credential browser-profile namespace, mirrored into a cookie so the
/// server can render the device-scoped workspace on the first paint.
const DEVICE_COOKIE: &str = "rumahl_device";
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
    pub apps: Option<Arc<crate::AppAccess>>,
    pub files: Option<Arc<dyn rumahl_core::PersonalFiles>>,
    pub workspace: Option<Arc<dyn rumahl_core::WorkspaceRepository>>,
    pub preferences: Option<Arc<dyn rumahl_core::ShellPreferencesRepository>>,
    /// Exposure-mode policy; None keeps the guided default.
    pub os_mode: Option<Arc<dyn rumahl_core::OsModeRepository>>,
    /// Real host roots the explorer may browse; None disables them.
    pub host_files: Option<Arc<dyn crate::HostFiles>>,
    /// None leaves the Shell and recovery routes usable while OIDC is offline.
    pub oidc: Option<Arc<dyn oidc::OidcGateway>>,
    pub browser_sessions: Option<Arc<dyn crate::BrowserSessions>>,
    pub login_slots: Arc<tokio::sync::Semaphore>,
    /// Central append-only audit log; None disables auditing without disabling the shell.
    pub audit: Option<Arc<dyn rumahl_core::AuditLog>>,
}

/// Current wall-clock time as an audit timestamp.
fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Best-effort central audit record. Auditing must never fail a request.
pub(crate) fn record_audit(
    state: &GatewayState,
    actor: rumahl_core::AuditActor,
    action: rumahl_core::AuditAction,
    outcome: rumahl_core::AuditOutcome,
    target: Option<&str>,
) {
    let Some(audit) = &state.audit else { return };
    let at = rumahl_core::UnixTimestamp::from_seconds(now_seconds());
    let event = match target {
        Some(target) => {
            let base = rumahl_core::NewAuditEvent::new(at, actor.clone(), action, outcome);
            base.with_target(target.to_owned())
                .unwrap_or_else(|_| rumahl_core::NewAuditEvent::new(at, actor, action, outcome))
        }
        None => rumahl_core::NewAuditEvent::new(at, actor, action, outcome),
    };
    let _ = audit.record(event);
}

/// Records every state-changing API action in the central audit log, so the
/// activity monitor sees everything the OS does and not only named events.
/// Best-effort: auditing never changes the response.
async fn audit_activity(
    State(state): State<Arc<GatewayState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    use axum::http::Method;
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let mutating = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let token = if mutating {
        credential(request.headers()).ok()
    } else {
        None
    };
    let response = next.run(request).await;
    if mutating && path.starts_with("/api/v1/") {
        let actor = match token {
            Some(token) => match authenticate(&state, &token).await {
                Ok(identity) => rumahl_core::AuditActor::User(identity.user_id),
                Err(_) => rumahl_core::AuditActor::System,
            },
            None => rumahl_core::AuditActor::System,
        };
        let outcome = if response.status().is_success() {
            rumahl_core::AuditOutcome::Success
        } else if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            rumahl_core::AuditOutcome::Denied
        } else {
            rumahl_core::AuditOutcome::Failure
        };
        record_audit(
            &state,
            actor,
            rumahl_core::AuditAction::Activity,
            outcome,
            Some(&format!("{method} {path}")),
        );
    }
    response
}

pub fn router(state: GatewayState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/", get(shell))
        .fallback(shell_fallback)
        .route("/api/v1/shell/snapshot", get(snapshot))
        .route("/api/v1/shell/apps", get(app_catalog))
        .route(
            "/api/v1/shell/apps/{id}/launch",
            post(app_launch).layer(axum::extract::DefaultBodyLimit::max(1024)),
        )
        .route("/api/v1/shell/apps/{id}/settings", get(app_settings))
        .route("/api/v1/shell/apps/{id}/data", get(app_data))
        .route(
            "/api/v1/shell/apps/{id}/runtime",
            get(app_runtime_status).post(app_runtime_action),
        )
        .route(
            "/api/v1/shell/apps/import",
            post(app_import).layer(axum::extract::DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route(
            "/api/v1/shell/capabilities/invoke",
            post(capabilities_invoke).layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
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
        .route("/api/v1/recovery/status", get(recovery_status))
        .route("/api/v1/recovery/audit", get(recovery_audit))
        .route("/api/v1/recovery/components", get(recovery_components))
        .route("/api/v1/recovery/sensors", get(recovery_sensors))
        .route("/api/v1/recovery/restart", post(recovery_restart))
        .route("/api/v1/recovery/self-heal", post(recovery_self_heal))
        .route(
            "/api/v1/shell/report",
            post(shell_report).layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .merge(oidc::routes())
        .merge(crate::preferences::routes())
        .merge(crate::os_mode::routes())
        .merge(crate::host_files::routes())
        .merge(crate::workspace::routes())
        .merge(crate::files::routes())
        .merge(crate::browser_auth::routes())
        .merge(crate::offline::routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            audit_activity,
        ))
        .with_state(state)
}

/// Expose the router only on a private local socket. A separate HTTPS edge must
/// preserve Host, Origin and Cookie; it must not add trusted identity headers.
pub async fn serve(socket: &Path, state: GatewayState) -> Result<(), GatewayError> {
    serve_router(socket, router(state), false).await
}

/// Group access is explicit and must match the private parent directory group.
/// The default `serve` entry point retains owner-only access.
pub async fn serve_router(
    socket: &Path,
    app: Router,
    group_access: bool,
) -> Result<(), GatewayError> {
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
    if group_access
        && (metadata.permissions().mode() & 0o010 == 0
            || std::fs::symlink_metadata(socket)
                .map_err(GatewayError::Io)?
                .gid()
                != metadata.gid())
    {
        return Err(GatewayError::InvalidSocketPath);
    }
    std::fs::set_permissions(
        socket,
        std::fs::Permissions::from_mode(if group_access { 0o660 } else { 0o600 }),
    )
    .map_err(GatewayError::Io)?;
    axum::serve(listener, app).await.map_err(GatewayError::Io)
}

async fn app_catalog(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || apps.catalog(identity)).await {
        Ok(Ok(apps)) => {
            let apps: Vec<_> = apps.into_iter().map(|app| serde_json::json!({
                "id": app.id, "installationId": app.installation_id.to_string(), "title": app.title,
                "version": app.version, "launchable": app.launchable, "capabilities": app.capabilities,
                "lifecycle": app.lifecycle, "connectors": app.connectors,
            })).collect();
            let mut response =
                axum::Json(serde_json::json!({"catalogVersion": 1, "apps": apps})).into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn app_launch(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 1024 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| key != "installationId" && key != "lease")
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = value
        .get("installationId")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let lease = match value.get("lease") {
        None => None,
        Some(serde_json::Value::String(s)) if s.len() == 64 => Some(s.clone()),
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || {
        apps.launch(identity, &id, &installation, lease.as_deref())
    })
    .await
    {
        Ok(Ok(launch)) => {
            let mut response = axum::Json(serde_json::json!({"launchVersion": 1, "id": launch.app.id, "installationId": launch.app.installation_id.to_string(), "lease": launch.lease, "frameUrl": launch.frame_url, "renewAfterSeconds": 30})).into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn app_settings(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = query.get("installationId") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let installation = installation.clone();
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || apps.settings(identity, &id, &installation)).await {
        Ok(Ok(settings)) => {
            let manifest: Vec<_> = settings
                .manifest
                .into_iter()
                .map(|setting| {
                    let options: Vec<_> = setting
                        .options
                        .into_iter()
                        .map(|option| {
                            serde_json::json!({"value": option.value, "label": option.label})
                        })
                        .collect();
                    serde_json::json!({
                        "key": setting.key,
                        "title": setting.title,
                        "description": setting.description,
                        "type": setting.kind,
                        "required": setting.required,
                        "options": options,
                    })
                })
                .collect();
            let mut response = axum::Json(serde_json::json!({
                "settingsVersion": 1,
                "id": settings.app.id,
                "installationId": settings.app.installation_id.to_string(),
                "manifest": manifest,
                "extended": serde_json::Value::Null,
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Read-only access to an app's private data directory. Only the app owner may
/// read it, and only in advanced mode or above.
async fn app_data(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    if query.len() < 2 || query.len() > 3 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = query.get("installationId").cloned() else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(device) = query
        .get("device")
        .and_then(|value| BrowserProfileId::parse(value))
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let path = query.get("path").cloned().unwrap_or_default();

    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user = identity.user_id;
    let mode = match tokio::task::spawn_blocking(move || mode_repository.load(user, device)).await {
        Ok(Ok(settings)) => settings.effective_mode(),
        Ok(Err(_)) => return error(StatusCode::SERVICE_UNAVAILABLE),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    if !mode.policy().can_browse_system_files() {
        return error(StatusCode::FORBIDDEN);
    }

    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let requested = path.clone();
    match tokio::task::spawn_blocking(move || apps.data(identity, &id, &installation, &requested))
        .await
    {
        Ok(Ok(data)) => {
            let value = match data {
                crate::AppData::Directory(entries) => serde_json::json!({
                    "dataVersion": 1,
                    "kind": "directory",
                    "path": path,
                    "entries": entries.into_iter().map(|entry| serde_json::json!({
                        "name": entry.name, "directory": entry.directory, "size": entry.size,
                    })).collect::<Vec<_>>(),
                }),
                crate::AppData::File {
                    bytes,
                    content_type,
                } => {
                    let text = if bytes.len() <= 256 * 1024 {
                        std::str::from_utf8(&bytes).ok().map(str::to_owned)
                    } else {
                        None
                    };
                    serde_json::json!({
                        "dataVersion": 1,
                        "kind": "file",
                        "path": path,
                        "size": bytes.len(),
                        "contentType": content_type,
                        "text": text,
                    })
                }
            };
            let mut response = axum::Json(value).into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn app_runtime_status(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = query.get("installationId").cloned() else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let app_id = id.clone();
    let installation_id = installation.clone();
    match tokio::task::spawn_blocking(move || {
        apps.runtime(
            identity,
            &app_id,
            &installation_id,
            crate::RuntimeAction::Status,
        )
    })
    .await
    {
        Ok(Ok(runtime)) => {
            let mut response = axum::Json(serde_json::json!({
                "runtimeVersion": 1,
                "id": id,
                "installationId": installation,
                "state": runtime.as_str(),
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn app_runtime_action(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 1024 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| key != "installationId" && key != "action")
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = value
        .get("installationId")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let action = match value.get("action").and_then(|v| v.as_str()) {
        Some("start") => crate::RuntimeAction::Start,
        Some("stop") => crate::RuntimeAction::Stop,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let user = identity.user_id;
    let starting = matches!(action, crate::RuntimeAction::Start);
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let app_id = id.clone();
    let installation_id = installation.clone();
    match tokio::task::spawn_blocking(move || {
        apps.runtime(identity, &app_id, &installation_id, action)
    })
    .await
    {
        Ok(Ok(runtime)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                if starting {
                    rumahl_core::AuditAction::AppRuntimeStarted
                } else {
                    rumahl_core::AuditAction::AppRuntimeStopped
                },
                rumahl_core::AuditOutcome::Success,
                Some(id.as_str()),
            );
            let mut response = axum::Json(serde_json::json!({
                "runtimeVersion": 1,
                "id": id,
                "installationId": installation,
                "state": runtime.as_str(),
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                if starting {
                    rumahl_core::AuditAction::AppRuntimeStarted
                } else {
                    rumahl_core::AuditAction::AppRuntimeStopped
                },
                rumahl_core::AuditOutcome::Failure,
                Some(id.as_str()),
            );
            crate::app_error(error)
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn capabilities_invoke(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 4096 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| key != "capability" && key != "resource")
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(capability) = value
        .get("capability")
        .and_then(|v| v.as_str())
        .filter(|value| value.len() <= 200)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let resource = match value.get("resource") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Object(resource)) => {
            if resource
                .keys()
                .any(|key| key != "namespace" && key != "kind" && key != "key")
            {
                return error(StatusCode::BAD_REQUEST);
            }
            let (Some(namespace), Some(kind), Some(key)) = (
                resource.get("namespace").and_then(|v| v.as_str()),
                resource.get("kind").and_then(|v| v.as_str()),
                resource.get("key").and_then(|v| v.as_str()),
            ) else {
                return error(StatusCode::BAD_REQUEST);
            };
            Some(crate::CapabilityResource {
                namespace: namespace.to_owned(),
                kind: kind.to_owned(),
                key: key.to_owned(),
            })
        }
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let user = identity.user_id;
    let audit_capability = capability.clone();
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || {
        apps.invoke_capability(identity, capability, resource)
    })
    .await
    {
        Ok(Ok(result)) => {
            let denied = result.outcome.as_str() == "denied";
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                if denied {
                    rumahl_core::AuditAction::CapabilityDenied
                } else {
                    rumahl_core::AuditAction::CapabilityInvoked
                },
                if denied {
                    rumahl_core::AuditOutcome::Denied
                } else {
                    rumahl_core::AuditOutcome::Success
                },
                Some(audit_capability.as_str()),
            );
            let value = result
                .result
                .as_deref()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
            let browser = result.browser.as_ref().map(|delivery| {
                serde_json::json!({
                    "appId": delivery.app_id,
                    "installationId": delivery.installation_id,
                })
            });
            let mut response = axum::Json(serde_json::json!({
                "capabilityVersion": 1,
                "outcome": result.outcome.as_str(),
                "result": value,
                "browser": browser,
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                rumahl_core::AuditAction::CapabilityInvoked,
                rumahl_core::AuditOutcome::Failure,
                Some(audit_capability.as_str()),
            );
            crate::app_error(error)
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn app_import(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match credential(&headers) {
        Ok(token) => match authenticate(&state, &token).await {
            Ok(identity) => identity,
            Err(error) => return backend_error(error),
        },
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| key != "importVersion" && key != "files" && key != "device")
    {
        return error(StatusCode::BAD_REQUEST);
    }
    if value.get("importVersion").and_then(|v| v.as_i64()) != Some(1) {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(device) = value
        .get("device")
        .and_then(|v| v.as_str())
        .and_then(BrowserProfileId::parse)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    // Installing app packages requires an OS mode that permits it.
    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user = identity.user_id;
    let mode = match tokio::task::spawn_blocking(move || mode_repository.load(user, device)).await {
        Ok(Ok(settings)) => settings.effective_mode(),
        Ok(Err(_)) | Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    if !mode.policy().can_install_apps() {
        return error(StatusCode::FORBIDDEN);
    }
    let Some(files) = value.get("files").and_then(|v| v.as_array()) else {
        return error(StatusCode::BAD_REQUEST);
    };
    if files.is_empty() || files.len() > 2048 {
        return error(StatusCode::BAD_REQUEST);
    }
    let mut upload = Vec::with_capacity(files.len());
    for file in files {
        let Some(entry) = file.as_object() else {
            return error(StatusCode::BAD_REQUEST);
        };
        if entry.keys().any(|key| key != "path" && key != "content") {
            return error(StatusCode::BAD_REQUEST);
        }
        let Some(path) = entry.get("path").and_then(|v| v.as_str()) else {
            return error(StatusCode::BAD_REQUEST);
        };
        if path.is_empty()
            || path.len() > 1024
            || path
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return error(StatusCode::BAD_REQUEST);
        }
        let Some(content) = entry.get("content").and_then(|v| v.as_str()) else {
            return error(StatusCode::BAD_REQUEST);
        };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(content) else {
            return error(StatusCode::BAD_REQUEST);
        };
        upload.push(crate::PackageUploadFile {
            path: path.to_owned(),
            bytes,
        });
    }
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || apps.import_package(identity, upload)).await {
        Ok(Ok(app)) => {
            let mut response = axum::Json(serde_json::json!({
                "importVersion": 1,
                "id": app.id,
                "installationId": app.installation_id.to_string(),
                "title": app.title,
                "version": app.version,
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => crate::app_error(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn shell_fallback(
    State(state): State<Arc<GatewayState>>,
    OriginalUri(uri): OriginalUri,
    method: axum::http::Method,
    headers: HeaderMap,
) -> Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return error(StatusCode::METHOD_NOT_ALLOWED);
    }
    if !crate::browser_auth::valid_shell_target(&uri.to_string()) {
        return error(StatusCode::NOT_FOUND);
    }
    shell(State(state), OriginalUri(uri), headers).await
}

async fn shell(
    State(state): State<Arc<GatewayState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    if !crate::browser_auth::valid_shell_target(&uri.to_string()) {
        return error(StatusCode::BAD_REQUEST);
    }
    let Ok(credential) = credential(&headers) else {
        return if state.browser_sessions.is_some() {
            crate::browser_auth::login_redirect_to(&uri.to_string())
        } else {
            error(StatusCode::UNAUTHORIZED)
        };
    };
    let identity = match authenticate(&state, &credential).await {
        Ok(identity) => identity,
        Err(ShellBackendError::Unauthorized) if state.browser_sessions.is_some() => {
            return crate::browser_auth::login_redirect_to(&uri.to_string());
        }
        Err(error) => return backend_error(error),
    };
    let snapshot = match load_snapshot(&state, &credential, device_cookie(&headers)).await {
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
        &uri.to_string(),
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
    let snapshot = match load_snapshot(&state, &credential, device_cookie(&headers)).await {
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
    let snapshot = match load_snapshot(&state, &credential, device_cookie(&headers)).await {
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

fn device_cookie(headers: &HeaderMap) -> Option<BrowserProfileId> {
    let cookie = headers.get(COOKIE)?.to_str().ok()?;
    let mut found: Option<&str> = None;
    for pair in cookie.split(';') {
        let Some((name, value)) = pair.trim().split_once('=') else {
            continue;
        };
        if name == DEVICE_COOKIE {
            if found.is_some() {
                return None;
            }
            found = Some(value);
        }
    }
    found.and_then(BrowserProfileId::parse)
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
                match authenticate(&state, &credential).await {
                    Ok(current) if current == identity => {},
                    Err(ShellBackendError::Unauthorized) => {
                        let _ = socket.send(Message::Text(ShellEvent::session_revoked().to_json().into())).await;
                        break;
                    }
                    _ => break,
                }
            }
            next = subscriber.recv() => match next {
                Ok((user_id, event)) if user_id == identity.user_id => {
                    match authenticate(&state, &credential).await {
                    Ok(current) if current == identity => {},
                    Err(ShellBackendError::Unauthorized) => {
                        let _ = socket.send(Message::Text(ShellEvent::session_revoked().to_json().into())).await;
                        break;
                    }
                    _ => break,
                }
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

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Formats a Unix timestamp as `YYYY-MM-DD HH:MM:SSZ` (UTC) without extra deps.
fn format_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let secs = seconds % 86_400;
    let (hour, minute, second) = (secs / 3_600, (secs % 3_600) / 60, secs % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}Z")
}

fn audit_event_json(event: &rumahl_core::AuditEvent) -> serde_json::Value {
    serde_json::json!({
        "sequence": event.sequence,
        "at": event.at.as_seconds(),
        "actor": event.actor.encode(),
        "action": event.action.as_str(),
        "outcome": event.outcome.as_str(),
        "target": event.target,
    })
}

async fn recovery_identity(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
) -> Result<ShellIdentity, Response> {
    match credential(headers) {
        Ok(token) => authenticate(state, &token).await.map_err(backend_error),
        Err(_) => Err(error(StatusCode::UNAUTHORIZED)),
    }
}

async fn recovery_status(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let sensors = crate::sensors::read_sensors();
    let last = state
        .audit
        .as_ref()
        .and_then(|audit| audit.recent_system(1).ok())
        .and_then(|events| events.first().map(|event| event.sequence))
        .unwrap_or(0);
    let audit = if state.audit.is_some() {
        "ok"
    } else {
        "disabled"
    };
    let apps = if state.apps.is_some() {
        "ok"
    } else {
        "disabled"
    };
    let checks = serde_json::json!([
        {"id": "platform", "state": "ok"},
        {"id": "renderer", "state": "unknown"},
        {"id": "audit", "state": audit},
        {"id": "apps", "state": apps},
        {"id": "sensors", "state": sensors.level},
    ]);
    // Overall verdict: sensor severity wins, then a missing subsystem warns.
    let level = if sensors.level == "error" {
        "error"
    } else if sensors.level == "warning" || audit == "disabled" || apps == "disabled" {
        "warning"
    } else {
        "information"
    };
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1,
        "ok": level == "information",
        "level": level,
        "now": now_seconds(),
        "lastAuditSequence": last,
        "checks": checks,
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_audit(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let mut filter = rumahl_core::AuditQuery {
        limit: query
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(100)
            .min(500),
        offset: query
            .get("offset")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0),
        ..Default::default()
    };
    if let Some(value) = query.get("action") {
        match rumahl_core::AuditAction::parse(value) {
            Ok(action) => filter.action = Some(action),
            Err(_) => return error(StatusCode::BAD_REQUEST),
        }
    }
    if let Some(value) = query.get("actor") {
        match rumahl_core::AuditActor::decode(value) {
            Ok(actor) => filter.actor = Some(actor),
            Err(_) => return error(StatusCode::BAD_REQUEST),
        }
    }
    if let Some(value) = query.get("outcome") {
        match rumahl_core::AuditOutcome::parse(value) {
            Ok(outcome) => filter.outcome = Some(outcome),
            Err(_) => return error(StatusCode::BAD_REQUEST),
        }
    }
    if let Some(value) = query.get("target") {
        if value.is_empty() || value.len() > rumahl_core::MAX_TARGET_BYTES {
            return error(StatusCode::BAD_REQUEST);
        }
        filter.target = Some(value.clone());
    }
    if let Some(value) = query.get("since") {
        match value.parse::<u64>() {
            Ok(seconds) => filter.since = Some(rumahl_core::UnixTimestamp::from_seconds(seconds)),
            Err(_) => return error(StatusCode::BAD_REQUEST),
        }
    }
    let Some(audit) = &state.audit else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let events = match audit.query_system(&filter) {
        Ok(events) => events,
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    let next =
        (filter.limit > 0 && events.len() == filter.limit).then(|| filter.offset + events.len());
    let events: Vec<_> = events.iter().map(audit_event_json).collect();
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1,
        "events": events,
        "nextOffset": next,
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_components(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1,
        "components": [
            {"id": "shell", "kind": "client", "state": "running", "restartable": true},
            {"id": "platform", "kind": "service", "state": "running", "restartable": false},
            {"id": "renderer", "kind": "service", "state": "unknown", "restartable": false},
            {"id": "audit", "kind": "store", "state": if state.audit.is_some() { "running" } else { "disabled" }, "restartable": false},
            {"id": "apps", "kind": "supervisor", "state": if state.apps.is_some() { "running" } else { "disabled" }, "restartable": false},
        ],
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_sensors(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let mut response = axum::Json(crate::sensors::read_sensors()).into_response();
    secure_headers(&mut response);
    response
}

/// Guided restart. `target: "shell"` asks the client to reload (it always can);
/// `target: "app"` restarts one installed app's runtime through the supervisor.
async fn recovery_restart(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 4096 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| !["target", "appId", "installationId"].contains(&key.as_str()))
    {
        return error(StatusCode::BAD_REQUEST);
    }
    match value.get("target").and_then(|v| v.as_str()) {
        Some("shell") => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                rumahl_core::AuditAction::Maintenance,
                rumahl_core::AuditOutcome::Success,
                Some("shell"),
            );
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Some("app") => {
            let (Some(app_id), Some(installation_id)) = (
                value
                    .get("appId")
                    .and_then(|v| v.as_str())
                    .filter(|value| !value.is_empty() && value.len() <= 200)
                    .map(str::to_owned),
                value
                    .get("installationId")
                    .and_then(|v| v.as_str())
                    .filter(|value| !value.is_empty() && value.len() <= 200)
                    .map(str::to_owned),
            ) else {
                return error(StatusCode::BAD_REQUEST);
            };
            let Some(apps) = state.apps.clone() else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            };
            let user = identity.user_id;
            let session = identity.session_id;
            let audit_id = app_id.clone();
            match tokio::task::spawn_blocking(move || {
                apps.runtime(
                    identity,
                    &app_id,
                    &installation_id,
                    crate::RuntimeAction::Stop,
                )?;
                apps.runtime(
                    ShellIdentity {
                        user_id: user,
                        session_id: session,
                    },
                    &app_id,
                    &installation_id,
                    crate::RuntimeAction::Start,
                )
            })
            .await
            {
                Ok(Ok(runtime)) => {
                    record_audit(
                        &state,
                        rumahl_core::AuditActor::User(user),
                        rumahl_core::AuditAction::AppRuntimeStarted,
                        rumahl_core::AuditOutcome::Success,
                        Some(audit_id.as_str()),
                    );
                    let mut response = axum::Json(serde_json::json!({
                        "recoveryVersion": 1,
                        "ok": true,
                        "state": runtime.as_str(),
                    }))
                    .into_response();
                    secure_headers(&mut response);
                    response
                }
                Ok(Err(error)) => {
                    record_audit(
                        &state,
                        rumahl_core::AuditActor::User(user),
                        rumahl_core::AuditAction::AppRuntimeStarted,
                        rumahl_core::AuditOutcome::Failure,
                        Some(audit_id.as_str()),
                    );
                    crate::app_error(error)
                }
                Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
            }
        }
        _ => error(StatusCode::BAD_REQUEST),
    }
}

async fn recovery_self_heal(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    record_audit(
        &state,
        rumahl_core::AuditActor::User(identity.user_id),
        rumahl_core::AuditAction::SelfHeal,
        rumahl_core::AuditOutcome::Success,
        None,
    );
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1,
        "ok": true,
        "actions": ["maintenance run recorded"],
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

/// Every program/service reports its activity here (app launches, crashes,
/// unresponsiveness, stops). The record lands in the central audit log.
async fn shell_report(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object
        .keys()
        .any(|key| key != "appId" && key != "event" && key != "action" && key != "installationId")
    {
        return error(StatusCode::BAD_REQUEST);
    }
    // Generic shell activity (navigation, mode switch, window/appearance actions).
    if let Some(action) = value.get("action").and_then(|v| v.as_str()) {
        if action.is_empty() || action.len() > 200 {
            return error(StatusCode::BAD_REQUEST);
        }
        record_audit(
            &state,
            rumahl_core::AuditActor::User(identity.user_id),
            rumahl_core::AuditAction::Activity,
            rumahl_core::AuditOutcome::Success,
            Some(action),
        );
        let mut response = StatusCode::NO_CONTENT.into_response();
        secure_headers(&mut response);
        return response;
    }
    let Some(app_id) = value
        .get("appId")
        .and_then(|v| v.as_str())
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let (action, outcome) = match value.get("event").and_then(|v| v.as_str()) {
        Some("crashed") => (
            rumahl_core::AuditAction::AppCrashed,
            rumahl_core::AuditOutcome::Failure,
        ),
        Some("unresponsive") => (
            rumahl_core::AuditAction::AppUnresponsive,
            rumahl_core::AuditOutcome::Failure,
        ),
        Some("launch-failed") => (
            rumahl_core::AuditAction::AppLaunchFailed,
            rumahl_core::AuditOutcome::Failure,
        ),
        Some("launched") => (
            rumahl_core::AuditAction::AppLaunched,
            rumahl_core::AuditOutcome::Success,
        ),
        Some("stopped") => (
            rumahl_core::AuditAction::AppRuntimeStopped,
            rumahl_core::AuditOutcome::Success,
        ),
        _ => return error(StatusCode::BAD_REQUEST),
    };
    record_audit(
        &state,
        rumahl_core::AuditActor::App(app_id.clone()),
        action,
        outcome,
        Some(app_id.as_str()),
    );
    // Self-healing: a crashed or unresponsive app runtime is restarted for the
    // user, and the recovery is recorded as a rumahl OS action.
    if matches!(
        action,
        rumahl_core::AuditAction::AppCrashed | rumahl_core::AuditAction::AppUnresponsive
    ) && let Some(installation_id) = value
        .get("installationId")
        .and_then(|v| v.as_str())
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .map(str::to_owned)
        && let Some(apps) = state.apps.clone()
    {
        let user = identity.user_id;
        let session = identity.session_id;
        let audit_id = app_id.clone();
        let recovered = tokio::task::spawn_blocking(move || {
            apps.runtime(
                identity,
                &app_id,
                &installation_id,
                crate::RuntimeAction::Stop,
            )?;
            apps.runtime(
                ShellIdentity {
                    user_id: user,
                    session_id: session,
                },
                &app_id,
                &installation_id,
                crate::RuntimeAction::Start,
            )
        })
        .await;
        let (heal_action, heal_outcome) = match recovered {
            Ok(Ok(_)) => (
                rumahl_core::AuditAction::ServiceRecovered,
                rumahl_core::AuditOutcome::Success,
            ),
            _ => (
                rumahl_core::AuditAction::SelfHeal,
                rumahl_core::AuditOutcome::Failure,
            ),
        };
        record_audit(
            &state,
            rumahl_core::AuditActor::Os,
            heal_action,
            heal_outcome,
            Some(audit_id.as_str()),
        );
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    secure_headers(&mut response);
    response
}

/// The central emergency & maintenance hub. Rendered without the shell renderer
/// so it stays available when the shell is down. Shows system status and, when
/// signed in, the recent activity log.
async fn recovery(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let german = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("de"));
    let authenticated = match credential(&headers) {
        Ok(token) => authenticate(&state, &token).await.is_ok(),
        Err(_) => false,
    };
    let events = if authenticated {
        state
            .audit
            .as_ref()
            .and_then(|audit| audit.recent_system(40).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let rows = events
        .iter()
        .map(|event| {
            format!(
                "<tr><td class=\"seq\">{seq}</td><td class=\"at\">{at}</td><td class=\"actor\">{actor}</td><td class=\"action\">{action}</td><td class=\"outcome o-{outcome}\">{outcome}</td><td class=\"target\">{target}</td></tr>",
                seq = event.sequence,
                at = format_utc(event.at.as_seconds()),
                actor = escape_html(&event.actor.encode()),
                action = escape_html(event.action.as_str()),
                outcome = event.outcome.as_str(),
                target = escape_html(event.target.as_deref().unwrap_or("—")),
            )
        })
        .collect::<String>();
    let (
        lang,
        title,
        subtitle,
        status_label,
        audit_heading,
        empty_label,
        signin_label,
        reload_label,
    ) = if german {
        (
            "de",
            "Wiederherstellung",
            "Notfall- und Wartungssystem von rumahl OS",
            "Systemstatus",
            "Aktivitätsprotokoll",
            "Melde dich an, um das Protokoll zu sehen.",
            "Anmelden",
            "Shell erneut laden",
        )
    } else {
        (
            "en",
            "Recovery",
            "The rumahl OS emergency & maintenance system",
            "System status",
            "Activity log",
            "Sign in to view the activity log.",
            "Sign in",
            "Reload shell",
        )
    };
    let audit_body = if !authenticated {
        format!("<p class=\"muted\">{empty_label} <a href=\"/login\">{signin_label}</a></p>")
    } else if rows.is_empty() {
        format!("<p class=\"muted\">{empty_label}</p>")
    } else {
        format!(
            "<table><thead><tr><th>#</th><th>time</th><th>actor</th><th>action</th><th>result</th><th>target</th></tr></thead><tbody>{rows}</tbody></table>"
        )
    };
    let body = format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>rumahl OS · {title}</title><style>\
:root{{color-scheme:dark}}*{{box-sizing:border-box}}\
body{{margin:0;min-height:100vh;background:#000;border-top:3px solid #ff5a4d;color:#eef2ff;font:15px/1.5 ui-sans-serif,system-ui,\"Segoe UI\",Roboto,sans-serif}}\
main{{max-width:960px;margin:0 auto;padding:clamp(28px,6vh,72px) 20px 56px}}\
.badge{{display:inline-flex;align-items:center;gap:8px;padding:5px 12px;border-radius:999px;background:rgba(255,255,255,.08);border:1px solid rgba(255,255,255,.14);font-size:12.5px;letter-spacing:.02em}}\
.dot{{width:8px;height:8px;border-radius:50%;background:#35d07f;box-shadow:0 0 10px #35d07f}}\
h1{{margin:18px 0 6px;font-size:clamp(28px,4.4vw,40px);letter-spacing:-.01em}}\
p.sub{{margin:0 0 26px;color:#9aa6c4}}\
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:14px;margin:0 0 30px}}\
.card{{padding:16px 18px;border-radius:16px;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.1)}}\
.card h2{{margin:0 0 8px;font-size:12px;text-transform:uppercase;letter-spacing:.08em;color:#8ea0c6}}\
.card .val{{font-size:20px;font-weight:600}}\
.muted{{color:#8ea0c6}}\
.run{{display:flex;gap:10px;flex-wrap:wrap;margin:0 0 34px}}\
a.btn{{display:inline-block;padding:10px 18px;border-radius:999px;text-decoration:none;font-weight:600;font-size:14px;background:#3b6ff0;color:#fff;border:1px solid #4b7cf3}}\
a.btn.ghost{{background:rgba(255,255,255,.07);border-color:rgba(255,255,255,.16);color:#eef2ff}}\
h2.section{{font-size:15px;letter-spacing:.02em;margin:0 0 12px}}\
table{{width:100%;border-collapse:collapse;font-size:13.5px;border-radius:14px;overflow:hidden}}\
thead th{{text-align:left;padding:9px 12px;background:rgba(255,255,255,.06);color:#9aa6c4;font-weight:600;text-transform:uppercase;letter-spacing:.05em;font-size:11px}}\
tbody td{{padding:9px 12px;border-top:1px solid rgba(255,255,255,.07);vertical-align:top}}\
td.seq,td.at{{white-space:nowrap;color:#8ea0c6;font-variant-numeric:tabular-nums}}\
td.action{{font-weight:600}}\
td.target{{color:#c7d0ea;word-break:break-all}}\
.o-success{{color:#4fd08a}}.o-denied{{color:#f0b64b}}.o-failure{{color:#f07b6b}}\
footer{{margin-top:34px;color:#66708c;font-size:12.5px}}\
</style></head><body><main>\
<span class=\"badge\"><span class=\"dot\"></span>rumahl OS</span>\
<h1>{title}</h1><p class=\"sub\">{subtitle}</p>\
<div class=\"grid\">\
<div class=\"card\"><h2>{status_label}</h2><div class=\"val\">OK · platform</div><div class=\"muted\">renderer unknown · audit {audit_state}</div></div>\
<div class=\"card\"><h2>now</h2><div class=\"val\">{now}</div></div>\
<div class=\"card\"><h2>last event</h2><div class=\"val\">#{last}</div></div>\
</div>\
<div class=\"run\"><a class=\"btn\" href=\"/\">{reload_label}</a><a class=\"btn ghost\" href=\"/login\">{signin_label}</a></div>\
<h2 class=\"section\">{audit_heading}</h2>{audit_body}\
<footer>rumahl OS recovery — works without app containers or the shell renderer.</footer>\
</main></body></html>",
        lang = lang,
        title = title,
        subtitle = subtitle,
        status_label = status_label,
        audit_heading = audit_heading,
        now = format_utc(now_seconds()),
        last = state
            .audit
            .as_ref()
            .and_then(|audit| audit.recent_system(1).ok())
            .and_then(|events| events.first().map(|event| event.sequence))
            .unwrap_or(0),
        audit_state = if state.audit.is_some() { "on" } else { "off" },
        reload_label = reload_label,
        signin_label = signin_label,
        audit_body = audit_body,
    );
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    secure_headers(&mut response);
    response
}

pub(super) fn credential(headers: &HeaderMap) -> Result<Zeroizing<String>, ()> {
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

pub(super) fn valid_origin(headers: &HeaderMap, expected: &Url) -> bool {
    headers.get_all(ORIGIN).iter().count() == 1
        && headers.get(ORIGIN).and_then(|value| value.to_str().ok())
            == Some(expected.origin().ascii_serialization().as_str())
}

pub(super) async fn authenticate(
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
    device: Option<BrowserProfileId>,
) -> Result<ShellSnapshot, ShellBackendError> {
    let backend = Arc::clone(&state.backend);
    let credential = Zeroizing::new(credential.to_owned());
    tokio::task::spawn_blocking(move || backend.snapshot(&credential, device))
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
    let mut result: Vec<_> = origins.into_iter().take(31).collect();
    if let Some(apps) = &state.apps {
        result.push(apps.frame_source());
    }
    result
}

pub(super) fn secure_headers(response: &mut Response) {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    // Preserve Origin on same-origin HTML form POSTs. `no-referrer` makes
    // browsers send Origin: null, which correctly fails our CSRF check.
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
}

fn error(status: StatusCode) -> Response {
    if status == StatusCode::SERVICE_UNAVAILABLE {
        return unavailable_page();
    }
    let mut response = Response::new(Body::from("Request unavailable."));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

/// The shell could not be served. This stays dependency-free (no renderer, no
/// assets) and points to the recovery hub, which runs in the same process.
fn unavailable_page() -> Response {
    let body = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>rumahl OS · Unavailable</title><style>\
:root{color-scheme:dark}*{box-sizing:border-box}\
body{margin:0;min-height:100vh;display:grid;place-items:center;background:radial-gradient(900px 500px at 50% -10%,#20283a,#080b12 62%);color:#eef2ff;font:15px/1.55 ui-sans-serif,system-ui,\"Segoe UI\",Roboto,sans-serif;padding:24px}\
.card{max-width:460px;text-align:center;padding:36px 30px;border-radius:22px;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.12);box-shadow:0 24px 70px rgba(0,0,0,.5)}\
svg{width:52px;height:52px;stroke:#f0b64b;fill:none;stroke-width:1.5}\
h1{margin:16px 0 8px;font-size:26px;letter-spacing:-.01em}\
p{margin:0 0 22px;color:#9aa6c4}\
.actions{display:flex;gap:10px;justify-content:center;flex-wrap:wrap}\
a{display:inline-block;padding:11px 20px;border-radius:999px;text-decoration:none;font-weight:600;font-size:14px}\
a.primary{background:#3b6ff0;color:#fff}\
a.ghost{background:rgba(255,255,255,.07);color:#eef2ff;border:1px solid rgba(255,255,255,.16)}\
</style></head><body><div class=\"card\">\
<svg viewBox=\"0 0 24 24\" aria-hidden=\"true\"><path d=\"M12 3 3 6v6c0 4.5 3.2 7.9 9 9 5.8-1.1 9-4.5 9-9V6z\"/><path d=\"M12 8v5M12 16.5h.01\"/></svg>\
<h1>Shell unavailable</h1>\
<p>The shell could not be started right now. Recovery and maintenance remain available.</p>\
<div class=\"actions\"><a class=\"primary\" href=\"/recovery\">Open recovery</a><a class=\"ghost\" href=\"/\">Reload shell</a></div>\
</div></body></html>";
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
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
        ResolvedTheme, ShellSystemStatus, ShellTheme, ShellUser, SystemProtectionStatus,
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

        fn snapshot(
            &self,
            token: &str,
            _device: Option<BrowserProfileId>,
        ) -> Result<ShellSnapshot, ShellBackendError> {
            if !self.available || token != TOKEN {
                return Err(ShellBackendError::Unavailable);
            }
            Ok(ShellSnapshot::new(
                "shell-build-001", "revision-001",
                ShellUser::new("Example User", "en-US").unwrap(),
                ShellTheme::from_theme(
                    "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
                    &ResolvedTheme::stock(),
                ).unwrap(),
                Vec::new(),
                None,
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
            host_files: None,
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
            apps: None,
            preferences: None,
            os_mode: None,
            workspace: None,
            files: None,
            oidc: None,
            browser_sessions: None,
            login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            audit: None,
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
            host_files: None,
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestBackend {
                identity,
                available: true,
            }),
            events: Arc::new(crate::InMemoryShellEvents::new(4)),
            widgets: Arc::new(TestWidgets("https://weather.apps.rumahl.dev/widget")),
            streams: Some(Arc::new(StreamAccess::new(provider))),
            apps: None,
            preferences: None,
            os_mode: None,
            workspace: None,
            files: None,
            oidc: None,
            browser_sessions: None,
            login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            audit: None,
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
            host_files: None,
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestBackend {
                identity,
                available: true,
            }),
            events: Arc::new(crate::InMemoryShellEvents::new(4)),
            widgets: Arc::new(TestWidgets("https://weather.apps.rumahl.dev/widget")),
            streams: Some(Arc::new(StreamAccess::new(provider))),
            apps: None,
            preferences: None,
            os_mode: None,
            workspace: None,
            files: None,
            oidc: None,
            browser_sessions: None,
            login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            audit: None,
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
    async fn recovery_status_components_and_audit_serve_the_health_surface() {
        let app = router(state(true, "https://weather.apps.rumahl.dev/widget"));
        for uri in ["/api/v1/recovery/status", "/api/v1/recovery/components"] {
            assert_eq!(
                app.clone().oneshot(request(uri)).await.unwrap().status(),
                StatusCode::OK,
                "{uri}"
            );
        }
        let body = axum::body::to_bytes(
            app.clone()
                .oneshot(request("/api/v1/recovery/components"))
                .await
                .unwrap()
                .into_body(),
            8192,
        )
        .await
        .unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("platform"));
        // Filter validation happens before the store lookup.
        assert_eq!(
            app.clone()
                .oneshot(request("/api/v1/recovery/audit?action=nonsense"))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        // Without an audit store the read fails closed, not open.
        assert_eq!(
            app.oneshot(request("/api/v1/recovery/audit?limit=5"))
                .await
                .unwrap()
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
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
