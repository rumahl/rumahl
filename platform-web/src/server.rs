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
use axum::routing::{delete, get, post};
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
use crate::events::{ShellEventMessage, ShellEventSource};
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
    /// State directory for recovery settings/diagnostics; None disables them.
    pub state_dir: Option<std::path::PathBuf>,
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
            "/api/v1/shell/apps/{id}",
            delete(app_uninstall).layer(axum::extract::DefaultBodyLimit::max(2048)),
        )
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
        .route("/recovery.js", get(recovery_script))
        .route("/api/v1/recovery/status", get(recovery_status))
        .route("/api/v1/recovery/audit", get(recovery_audit))
        .route("/api/v1/recovery/components", get(recovery_components))
        .route("/api/v1/recovery/sensors", get(recovery_sensors))
        .route("/api/v1/recovery/storage", get(recovery_storage))
        .route("/api/v1/recovery/disks", get(recovery_disks))
        .route("/api/v1/recovery/disk-test", post(recovery_disk_test))
        .route("/api/v1/recovery/diagnostics", get(recovery_diagnostics))
        .route("/api/v1/recovery/network", get(recovery_network))
        .route("/api/v1/recovery/network-test", post(recovery_network_test))
        .route("/api/v1/recovery/logs", get(recovery_logs))
        .route("/api/v1/recovery/crashes", get(recovery_crashes))
        .route("/api/v1/recovery/boot", get(recovery_boot))
        .route("/api/v1/recovery/system", get(recovery_system))
        .route("/api/v1/recovery/quarantine", post(recovery_quarantine))
        .route(
            "/api/v1/recovery/settings",
            get(recovery_settings_get).put(recovery_settings_put),
        )
        .route(
            "/api/v1/recovery/host",
            get(recovery_host_status).post(recovery_host_action),
        )
        .route("/api/v1/recovery/restart", post(recovery_restart))
        .route("/api/v1/recovery/self-heal", post(recovery_self_heal))
        .route(
            "/api/v1/shell/report",
            post(shell_report).layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .merge(oidc::routes())
        .merge(crate::preferences::routes())
        .merge(crate::os_mode::routes())
        .merge(crate::store::routes())
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
                "runtime": app.runtime, "lifecycle": app.lifecycle, "connectors": app.connectors,
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
    if crate::recovery_settings::load(state.state_dir.as_deref()).safe_mode {
        let mut response =
            axum::Json(serde_json::json!({"launchError": "safe-mode"})).into_response();
        *response.status_mut() = StatusCode::LOCKED;
        secure_headers(&mut response);
        return response;
    }
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

async fn app_uninstall(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
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
    if object.keys().any(|key| key != "installationId") {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = value
        .get("installationId")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user = identity.user_id;
    let audit_id = id.clone();
    match tokio::task::spawn_blocking(move || apps.uninstall(identity, &id, &installation)).await {
        Ok(Ok(())) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                rumahl_core::AuditAction::AppUninstalled,
                rumahl_core::AuditOutcome::Success,
                Some(audit_id.as_str()),
            );
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(user),
                rumahl_core::AuditAction::AppUninstalled,
                rumahl_core::AuditOutcome::Failure,
                Some(audit_id.as_str()),
            );
            crate::app_error(error)
        }
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
    if starting && crate::recovery_settings::load(state.state_dir.as_deref()).safe_mode {
        return error(StatusCode::FORBIDDEN);
    }
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
    if object.keys().any(|key| {
        key != "importVersion" && key != "files" && key != "device" && key != "installOrigin"
    }) {
        return error(StatusCode::BAD_REQUEST);
    }
    let store_install = match value.get("installOrigin") {
        None => false,
        Some(origin) if origin.as_str() == Some("store") => true,
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
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
    // Manual package-folder imports require Advanced. Store installs are the
    // guided-mode exception: they are explicit, first-party UI actions and the
    // package still passes signature and payload verification.
    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user = identity.user_id;
    let mode = match tokio::task::spawn_blocking(move || mode_repository.load(user, device)).await {
        Ok(Ok(settings)) => settings.effective_mode(),
        Ok(Err(_)) | Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    if !(mode.policy().can_install_apps() || store_install && mode.policy().is_guided()) {
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
    mut subscriber: broadcast::Receiver<ShellEventMessage>,
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
                Ok(message) => {
                    let (target, event) = match message {
                        ShellEventMessage::ForUser(user_id, event) => (Some(user_id), event),
                        ShellEventMessage::ForAll(event) => (None, event),
                    };
                    if target.is_some_and(|user_id| user_id != identity.user_id) {
                        // Targeted at another account; not for this socket.
                    } else {
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
                }
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
            {"id": "shell", "kind": "client", "state": "unknown", "restartable": true},
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

async fn recovery_storage(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let mut response = axum::Json(crate::sensors::read_storage()).into_response();
    secure_headers(&mut response);
    response
}

/// Detects concrete problems the recovery environment can help resolve.
async fn recovery_diagnostics(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
) -> Response {
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let mut problems: Vec<serde_json::Value> = Vec::new();
    if state.audit.is_none() {
        problems.push(serde_json::json!({
            "id": "audit", "severity": "warning",
            "title": "The activity log is disabled",
            "detail": "The audit store is not available, so actions cannot be recorded.",
            "fix": null
        }));
    }
    if state.apps.is_none() {
        problems.push(serde_json::json!({
            "id": "apps", "severity": "warning",
            "title": "The app runtime is unavailable",
            "detail": "Installed applications cannot be started because the runtime supervisor is not connected.",
            "fix": null
        }));
    }
    if let Some(apps) = state.apps.clone() {
        let checks = tokio::task::spawn_blocking(move || {
            let catalog = apps.catalog(identity)?;
            let mut checks = Vec::new();
            for app in catalog
                .into_iter()
                .filter(|app| app.launchable && app.lifecycle == "always-on")
            {
                let status = apps.runtime(
                    identity,
                    &app.id,
                    &app.installation_id.to_string(),
                    crate::RuntimeAction::Status,
                );
                if !matches!(status, Ok(crate::RuntimeState::Running)) {
                    checks.push(serde_json::json!({
                        "id": "app-service", "severity": "warning",
                        "title": app.title,
                        "detail": if status.is_ok() { "Always-on service is stopped" } else { "Service status unavailable or denied" },
                        "fix": if status.is_ok() { Some("self-heal") } else { None },
                    }));
                }
            }
            Ok::<_, crate::AppAccessError>(checks)
        })
        .await;
        match checks {
            Ok(Ok(checks)) => problems.extend(checks),
            _ => problems.push(serde_json::json!({
                "id": "app-check", "severity": "warning",
                "title": "App services could not be checked",
                "detail": "Catalog access is unavailable or denied.", "fix": null,
            })),
        }
    }
    let sensors = crate::sensors::read_sensors();
    match sensors.level {
        "error" => problems.push(serde_json::json!({
            "id": "sensors", "severity": "error",
            "title": "Critical resource pressure",
            "detail": "CPU load, memory or temperature is at a critical level.",
            "fix": null
        })),
        "warning" => problems.push(serde_json::json!({
            "id": "sensors", "severity": "warning",
            "title": "High resource pressure",
            "detail": "A resource is running high and should be watched.",
            "fix": null
        })),
        _ => {}
    }
    let storage = crate::sensors::read_storage();
    if let (Some(level), Some(percent)) = (Some(storage.level), storage.percent)
        && (storage.level == "error" || storage.level == "warning")
    {
        problems.push(serde_json::json!({
            "id": "storage", "severity": level,
            "title": if storage.level == "error" { "The disk is almost full" } else { "The disk is filling up" },
            "detail": format!("The root filesystem is {percent:.0}% full."),
            "fix": "open-storage"
        }));
    }
    // Apps that crashed or became unresponsive recently (from the audit log).
    if let Some(audit) = &state.audit
        && let Ok(events) = audit.recent_system(200)
    {
        let mut crashed: Vec<String> = Vec::new();
        for event in &events {
            if matches!(
                event.action,
                rumahl_core::AuditAction::AppCrashed | rumahl_core::AuditAction::AppUnresponsive
            ) && let Some(target) = event.target.as_ref()
                && !crashed.contains(target)
            {
                crashed.push(target.clone());
            }
        }
        if !crashed.is_empty() {
            let severity = if crashed.len() >= 3 {
                "error"
            } else {
                "warning"
            };
            problems.push(serde_json::json!({
                "id": "app-crashes", "severity": severity,
                "title": format!("{} app(s) stopped unexpectedly", crashed.len()),
                "detail": crashed.join(", "),
                "fix": "restart-apps"
            }));
            if crashed.len() >= 3 {
                problems.push(serde_json::json!({
                    "id": "safe-mode", "severity": "warning",
                    "title": "Repeated app crashes",
                    "detail": "Safe mode keeps third-party apps stopped so the system can stabilise.",
                    "fix": "open-settings"
                }));
            }
        }
    }
    let mut response = axum::Json(serde_json::json!({ "problems": problems })).into_response();
    secure_headers(&mut response);
    response
}

/// Network interfaces, addresses, default gateway and resolvers. Read-only.
async fn recovery_network(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let data = tokio::task::spawn_blocking(crate::recovery_tools::read_network)
        .await
        .unwrap_or_else(|_| serde_json::json!({
            "recoveryVersion": 1, "interfaces": [], "gateway": null, "dns": [], "level": "warning",
        }));
    let mut response = axum::Json(data).into_response();
    secure_headers(&mut response);
    response
}

/// Active connectivity probes (gateway, resolver, public internet). Read-only.
async fn recovery_network_test(
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
    let data = tokio::task::spawn_blocking(crate::recovery_tools::run_network_test)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "recoveryVersion": 1, "ok": false, "checks": [] }));
    record_audit(
        &state,
        rumahl_core::AuditActor::User(identity.user_id),
        rumahl_core::AuditAction::Maintenance,
        rumahl_core::AuditOutcome::Success,
        Some("network-test"),
    );
    let mut response = axum::Json(data).into_response();
    secure_headers(&mut response);
    response
}

/// Recent platform or whole-system log lines, newest first.
async fn recovery_logs(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let source = match query
        .get("source")
        .map(String::as_str)
        .unwrap_or("platform")
    {
        "system" => "system",
        "platform" => "platform",
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let level = match query.get("level").map(String::as_str) {
        None | Some("") | Some("all") => None,
        Some("error") => Some("error"),
        Some("warning") => Some("warning"),
        Some("critical") => Some("critical"),
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(150)
        .clamp(1, 500);
    let offset = query
        .get("offset")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(5000);
    let data = tokio::task::spawn_blocking(move || {
        crate::recovery_tools::read_logs(source, level, limit, offset)
    })
    .await
    .unwrap_or_else(
        |_| serde_json::json!({ "recoveryVersion": 1, "available": false, "entries": [] }),
    );
    let mut response = axum::Json(data).into_response();
    secure_headers(&mut response);
    response
}

/// App crash, unresponsiveness and launch-failure history from the audit log.
async fn recovery_crashes(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let settings = crate::recovery_settings::load(state.state_dir.as_deref());
    let mut apps: Vec<serde_json::Value> = Vec::new();
    if let Some(audit) = &state.audit
        && let Ok(events) = audit.recent_system(500)
    {
        let mut index: BTreeMap<String, (u64, u64, u64, u64, String)> = BTreeMap::new();
        for event in &events {
            let (bucket, action) = match event.action {
                rumahl_core::AuditAction::AppCrashed => (0, "app-crashed"),
                rumahl_core::AuditAction::AppUnresponsive => (1, "app-unresponsive"),
                rumahl_core::AuditAction::AppLaunchFailed => (2, "app-launch-failed"),
                _ => continue,
            };
            let Some(target) = event.target.clone() else {
                continue;
            };
            let entry = index.entry(target).or_insert((0, 0, 0, 0, String::new()));
            match bucket {
                0 => entry.0 += 1,
                1 => entry.1 += 1,
                _ => entry.2 += 1,
            }
            entry.3 = entry.3.max(event.at.as_seconds());
            if entry.4.is_empty() {
                entry.4 = action.to_owned();
            }
        }
        apps = index
            .into_iter()
            .map(
                |(app_id, (crashes, unresponsive, launch_failed, last, _))| {
                    serde_json::json!({
                        "appId": app_id,
                        "crashes": crashes,
                        "unresponsive": unresponsive,
                        "launchFailed": launch_failed,
                        "last": last,
                    })
                },
            )
            .collect();
        apps.sort_by(|a, b| b["last"].as_u64().cmp(&a["last"].as_u64()));
    }
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1,
        "apps": apps,
        "disabled": settings.disabled_apps,
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

/// Failed units and boot errors explaining why recovery may be needed.
async fn recovery_boot(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let data = tokio::task::spawn_blocking(crate::recovery_tools::read_boot)
        .await
        .unwrap_or_else(|_| {
            serde_json::json!({
                "recoveryVersion": 1, "failedUnits": [], "errors": [],
            })
        });
    let mut response = axum::Json(data).into_response();
    secure_headers(&mut response);
    response
}

/// Build and system identification for the update/rollback lane. Read-only.
async fn recovery_system(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let data = tokio::task::spawn_blocking(crate::recovery_tools::read_system)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "recoveryVersion": 1 }));
    let mut response = axum::Json(data).into_response();
    secure_headers(&mut response);
    response
}

/// Quarantines (or releases) an app after repeated crashes. A quarantined app is
/// never auto-restarted; disabling also stops its runtime when possible.
async fn recovery_quarantine(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 1024 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers
        .get(CONTENT_TYPE)
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
        .any(|key| !["appId", "installationId", "disabled"].contains(&key.as_str()))
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(app_id) = value
        .get("appId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(disabled) = value.get("disabled").and_then(serde_json::Value::as_bool) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let mut settings = crate::recovery_settings::load(state.state_dir.as_deref());
    settings.disabled_apps.retain(|entry| entry != &app_id);
    if disabled {
        settings.disabled_apps.push(app_id.clone());
        if settings.disabled_apps.len() > 64 {
            settings.disabled_apps.remove(0);
        }
    }
    if !crate::recovery_settings::save(state.state_dir.as_deref(), &settings) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    if disabled
        && let Some(installation_id) = value
            .get("installationId")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 200)
            .map(str::to_owned)
        && let Some(apps) = state.apps.clone()
    {
        let audit_id = app_id.clone();
        let _ = tokio::task::spawn_blocking(move || {
            apps.runtime(
                identity,
                &app_id,
                &installation_id,
                crate::RuntimeAction::Stop,
            )
        })
        .await;
        record_audit(
            &state,
            rumahl_core::AuditActor::Os,
            rumahl_core::AuditAction::AppRuntimeStopped,
            rumahl_core::AuditOutcome::Success,
            Some(audit_id.as_str()),
        );
    }
    let user = identity.user_id;
    record_audit(
        &state,
        rumahl_core::AuditActor::User(user),
        rumahl_core::AuditAction::Maintenance,
        rumahl_core::AuditOutcome::Success,
        Some(if disabled {
            "quarantine app"
        } else {
            "release app"
        }),
    );
    let mut response = StatusCode::NO_CONTENT.into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_disks(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let mut response =
        axum::Json(serde_json::json!({ "devices": crate::sensors::read_disks() })).into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_disk_test(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 1024 {
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
    let Some(name) = value
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|value| !value.is_empty() && value.len() <= 128)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let test = value
        .get("test")
        .and_then(|v| v.as_str())
        .unwrap_or("short");
    let ok = crate::sensors::run_disk_test(name, test);
    record_audit(
        &state,
        rumahl_core::AuditActor::User(identity.user_id),
        rumahl_core::AuditAction::Maintenance,
        if ok {
            rumahl_core::AuditOutcome::Success
        } else {
            rumahl_core::AuditOutcome::Failure
        },
        Some("disk-test"),
    );
    let mut response = if ok {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error(StatusCode::SERVICE_UNAVAILABLE)
    };
    secure_headers(&mut response);
    response
}

/// Reads the recovery environment settings (auto-repair, safe mode).
async fn recovery_settings_get(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = recovery_identity(&state, &headers).await {
        return response;
    }
    let settings = crate::recovery_settings::load(state.state_dir.as_deref());
    let mut response = axum::Json(settings).into_response();
    secure_headers(&mut response);
    response
}

async fn recovery_settings_put(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > 1024 {
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
    let Ok(settings) = serde_json::from_slice::<crate::recovery_settings::RecoverySettings>(&body)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    if !crate::recovery_settings::save(state.state_dir.as_deref(), &settings) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    record_audit(
        &state,
        rumahl_core::AuditActor::User(identity.user_id),
        rumahl_core::AuditAction::Maintenance,
        rumahl_core::AuditOutcome::Success,
        Some("recovery settings"),
    );
    let mut response = StatusCode::NO_CONTENT.into_response();
    secure_headers(&mut response);
    response
}

/// Host-wide maintenance is authorized independently by a root-owned operator
/// allowlist in the broker. A valid browser session alone grants no host rights.
async fn recovery_host_status(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
) -> Response {
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    recovery_host_call(serde_json::json!({
        "action": "status", "userId": identity.user_id.to_string(),
    }))
    .await
}

async fn recovery_host_action(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let identity = match recovery_identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if body.len() > 1024 {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object_mut() else {
        return error(StatusCode::BAD_REQUEST);
    };
    let keys: &[&str] = match object.get("action").and_then(|value| value.as_str()) {
        Some("reboot")
            if object
                .get("safeMode")
                .is_some_and(serde_json::Value::is_boolean) =>
        {
            &["action", "safeMode"]
        }
        Some("backup") => &["action"],
        Some("restore")
            if object
                .get("backupId")
                .and_then(|value| value.as_str())
                .is_some_and(|id| {
                    id.len() == 32
                        && id
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                }) =>
        {
            &["action", "backupId"]
        }
        Some("fsck") | Some("reset") | Some("reinstall") | Some("rollback") | Some("poweroff") => {
            &["action"]
        }
        Some("schedule")
            if object
                .get("enabled")
                .is_some_and(serde_json::Value::is_boolean)
                && object
                    .get("intervalHours")
                    .and_then(serde_json::Value::as_u64)
                    .is_some()
                && object
                    .get("keep")
                    .and_then(serde_json::Value::as_u64)
                    .is_some() =>
        {
            &["action", "enabled", "intervalHours", "keep"]
        }
        _ => return error(StatusCode::BAD_REQUEST),
    };
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return error(StatusCode::BAD_REQUEST);
    }
    let action = object
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned();
    object.insert(
        "userId".into(),
        serde_json::json!(identity.user_id.to_string()),
    );
    let response = recovery_host_call(value).await;
    record_audit(
        &state,
        rumahl_core::AuditActor::User(identity.user_id),
        rumahl_core::AuditAction::Maintenance,
        if matches!(response.status(), StatusCode::ACCEPTED | StatusCode::OK) {
            rumahl_core::AuditOutcome::Success
        } else {
            rumahl_core::AuditOutcome::Denied
        },
        Some(&format!("host request: {action}")),
    );
    response
}

async fn recovery_host_call(value: serde_json::Value) -> Response {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = UnixStream::connect("/run/rumahl-recovery/control.sock").await?;
        // A replaced socket must not let an unprivileged service impersonate root.
        if stream.peer_cred()?.uid() != 0 {
            return Err(std::io::Error::other("untrusted recovery peer"));
        }
        let mut request = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
        request.push(b'\n');
        stream.write_all(&request).await?;
        let mut reader = BufReader::new(stream);
        let mut frame = Vec::new();
        loop {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                return Err(std::io::Error::other("incomplete recovery response"));
            }
            let length = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if frame.len() + length > 131_072 {
                return Err(std::io::Error::other("recovery response too large"));
            }
            let complete = available[length - 1] == b'\n';
            frame.extend_from_slice(&available[..length]);
            reader.consume(length);
            if complete {
                break;
            }
        }
        serde_json::from_slice::<serde_json::Value>(&frame).map_err(std::io::Error::other)
    })
    .await;
    let Ok(Ok(value)) = result else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let status = match value.get("status").and_then(|value| value.as_u64()) {
        Some(200) => StatusCode::OK,
        Some(202) => StatusCode::ACCEPTED,
        Some(400) => StatusCode::BAD_REQUEST,
        Some(403) => StatusCode::FORBIDDEN,
        Some(409) => StatusCode::CONFLICT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    let mut response = (status, axum::Json(value)).into_response();
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
            if crate::recovery_settings::load(state.state_dir.as_deref()).safe_mode {
                return error(StatusCode::FORBIDDEN);
            }
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
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let settings = crate::recovery_settings::load(state.state_dir.as_deref());
    let user = identity.user_id;
    let session = identity.session_id;
    let result = tokio::task::spawn_blocking(move || {
        let identity = || ShellIdentity {
            user_id: user,
            session_id: session,
        };
        let catalog = apps.catalog(identity())?;
        let mut actions = Vec::new();
        for app in catalog
            .into_iter()
            .filter(|app| app.launchable && app.lifecycle == "always-on")
        {
            if crate::recovery_settings::is_disabled(&settings, &app.id) {
                actions.push(serde_json::json!({
                    "appId": app.id, "outcome": "skipped", "detail": "app disabled",
                }));
                continue;
            }
            let installation = app.installation_id.to_string();
            let status = apps.runtime(
                identity(),
                &app.id,
                &installation,
                crate::RuntimeAction::Status,
            );
            let (outcome, detail) = match status {
                Ok(crate::RuntimeState::Running) => ("skipped", "already running"),
                Ok(crate::RuntimeState::Stopped) if settings.safe_mode => {
                    ("skipped", "safe mode enabled")
                }
                Ok(crate::RuntimeState::Stopped) => match apps.runtime(
                    identity(),
                    &app.id,
                    &installation,
                    crate::RuntimeAction::Start,
                ) {
                    Ok(crate::RuntimeState::Running) => ("success", "service started"),
                    _ => ("failure", "service could not be started"),
                },
                Err(_) => ("failure", "runtime status unavailable or denied"),
            };
            actions
                .push(serde_json::json!({ "appId": app.id, "outcome": outcome, "detail": detail }));
        }
        Ok::<_, crate::AppAccessError>(actions)
    })
    .await;
    let actions = match result {
        Ok(Ok(actions)) => actions,
        Ok(Err(err)) => return crate::app_error(err),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    for action in &actions {
        if action["outcome"] == "skipped" {
            continue;
        }
        record_audit(
            &state,
            rumahl_core::AuditActor::User(user),
            rumahl_core::AuditAction::SelfHeal,
            if action["outcome"] == "success" {
                rumahl_core::AuditOutcome::Success
            } else {
                rumahl_core::AuditOutcome::Failure
            },
            action["appId"].as_str(),
        );
    }
    let ok = !actions.iter().any(|action| action["outcome"] == "failure");
    record_audit(
        &state,
        rumahl_core::AuditActor::User(user),
        rumahl_core::AuditAction::SelfHeal,
        if ok {
            rumahl_core::AuditOutcome::Success
        } else {
            rumahl_core::AuditOutcome::Failure
        },
        Some("always-on services"),
    );
    let mut response = axum::Json(serde_json::json!({
        "recoveryVersion": 1, "ok": ok, "actions": actions,
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
    let auto_repair_settings = crate::recovery_settings::load(state.state_dir.as_deref());
    if matches!(
        action,
        rumahl_core::AuditAction::AppCrashed | rumahl_core::AuditAction::AppUnresponsive
    ) && auto_repair_settings.auto_repair
        && !auto_repair_settings.safe_mode
        && !crate::recovery_settings::is_disabled(&auto_repair_settings, &app_id)
        && let Some(installation_id) = value
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
async fn recovery(headers: HeaderMap) -> Response {
    let german = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("de"));
    let body = include_str!("recovery.html").replace("{{lang}}", if german { "de" } else { "en" });
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    secure_headers(&mut response);
    response
}

/// Standalone recovery controller, independent of the shell renderer.
async fn recovery_script() -> Response {
    let script = include_str!("recovery.js");
    let mut response = Response::new(Body::from(script));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; base-uri 'none'"),
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

pub(crate) fn error(status: StatusCode) -> Response {
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
    // Calm diagnostic screen (shared with the offline kit): black, the white
    // rumahl "r" top right, a spinner first and tips/next steps only after it
    // keeps failing. Self-contained, so it renders with nothing else available.
    let body = include_str!("offline/index.html");
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
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
            state_dir: None,
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
            state_dir: None,
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
            state_dir: None,
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
        let body = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("lang=\"de\""));
    }

    #[tokio::test]
    async fn host_maintenance_rejects_missing_auth_origin_and_unknown_commands() {
        let app = router(state(true, "https://weather.apps.rumahl.dev/widget"));
        for (cookie, origin, body, expected) in [
            (
                false,
                "https://rumahl.dev",
                r#"{"action":"backup"}"#,
                StatusCode::UNAUTHORIZED,
            ),
            (
                true,
                "https://evil.example",
                r#"{"action":"backup"}"#,
                StatusCode::FORBIDDEN,
            ),
            (
                true,
                "https://rumahl.dev",
                r#"{"action":"execute","command":"reboot"}"#,
                StatusCode::BAD_REQUEST,
            ),
            (
                true,
                "https://rumahl.dev",
                r#"{"action":"backup","userId":"other"}"#,
                StatusCode::BAD_REQUEST,
            ),
            (
                true,
                "https://rumahl.dev",
                r#"{"action":"restore","backupId":"../state"}"#,
                StatusCode::BAD_REQUEST,
            ),
            (
                true,
                "https://rumahl.dev",
                r#"{"action":"reboot","safeMode":"yes"}"#,
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri("/api/v1/recovery/host")
                .header(ORIGIN, origin)
                .header(CONTENT_TYPE, "application/json");
            if cookie {
                request = request.header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"));
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }

    #[tokio::test]
    async fn recovery_mutations_require_session_origin_and_real_runtime() {
        let app = router(state(true, "https://weather.apps.rumahl.dev/widget"));
        for (authenticated, origin, expected) in [
            (false, "https://rumahl.dev", StatusCode::UNAUTHORIZED),
            (true, "https://attacker.example", StatusCode::FORBIDDEN),
            (true, "https://rumahl.dev", StatusCode::SERVICE_UNAVAILABLE),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri("/api/v1/recovery/self-heal")
                .header(ORIGIN, origin);
            if authenticated {
                request = request.header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"));
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }

    #[tokio::test]
    async fn safe_mode_rejects_restarts_before_touching_runtime() {
        let directory = tempfile::tempdir().unwrap();
        assert!(crate::recovery_settings::save(
            Some(directory.path()),
            &crate::recovery_settings::RecoverySettings {
                auto_repair: true,
                safe_mode: true,
                disabled_apps: Vec::new(),
            },
        ));
        let mut gateway = state(true, "https://weather.apps.rumahl.dev/widget");
        gateway.state_dir = Some(directory.path().to_path_buf());
        let app = router(gateway);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recovery/restart")
                    .header(ORIGIN, "https://rumahl.dev")
                    .header(COOKIE, format!("{SESSION_COOKIE}={TOKEN}"))
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"target":"app","appId":"example.app","installationId":"unknown"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
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
        let body = axum::body::to_bytes(response.into_body(), 65536)
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
