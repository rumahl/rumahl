use crate::{
    GatewayState,
    preferences::{error, profile, storage_error, user},
    server::{secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use rumahl_core::{PreferenceScope, WorkspacePreferences};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Workspace {
    version: u8,
    windows: Vec<Window>,
    folders: Vec<Folder>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Window {
    location: String,
    rect: Rect,
    placement: String,
    minimized: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Folder {
    id: String,
    name: String,
    apps: Vec<String>,
}
fn valid(value: &Workspace) -> bool {
    let mut ids = HashSet::new();
    let mut apps = HashSet::new();
    value.version == 1
        && value.windows.len() <= 32
        && value.folders.len() <= 32
        && value.windows.iter().all(|w| {
            w.location.len() <= 2048
                && {
                    let path = w.location.split(['?', '#']).next().unwrap_or("");
                    path.starts_with("/app/")
                        || path.starts_with("/settings/")
                        || ["/settings", "/activity"].contains(&path)
                }
                && !w.location.contains(['\\', '\r', '\n'])
                && [w.rect.x, w.rect.y, w.rect.width, w.rect.height]
                    .iter()
                    .all(|n| n.is_finite() && *n >= 0.0 && *n <= 32768.0)
                && w.rect.width >= 1.0
                && w.rect.height >= 1.0
                && ["floating", "left", "right", "maximized"].contains(&w.placement.as_str())
        })
        && value.folders.iter().all(|f| {
            f.id.len() <= 64
                && !f.id.is_empty()
                && ids.insert(&f.id)
                && !f.name.trim().is_empty()
                && f.name.len() <= 128
                && !f.name.chars().any(char::is_control)
                && f.apps.len() <= 128
                && f.apps.iter().all(|a| {
                    !a.is_empty()
                        && a.len() <= 255
                        && a.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                        && apps.insert(a)
                })
        })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    revision: u64,
    scope: String,
    value: Option<Workspace>,
}
pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/workspace", get(load).put(save))
        .layer(DefaultBodyLimit::max(65536))
}
fn response(owner: rumahl_core::UserId, value: WorkspacePreferences) -> Response {
    let parse =
        |v: Option<String>| v.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    let mut r = axum::Json(serde_json::json!({"ownerId":owner.to_string(),"revision":value.revision,"user":parse(value.user),"device":parse(value.device)})).into_response();
    secure_headers(&mut r);
    r
}
async fn load(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let owner = match user(&state, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(device) = profile(&query) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repo) = state.workspace.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repo.load_workspace(owner, device)).await {
        Ok(Ok(v)) => response(owner, v),
        Ok(Err(e)) => storage_error(e),
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn save(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let owner = match user(&state, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(device) = profile(&query) else {
        return error(StatusCode::BAD_REQUEST);
    };
    if headers.get("content-type").and_then(|h| h.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(update) = serde_json::from_slice::<Update>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    if update.revision > 9_007_199_254_740_991 || update.value.as_ref().is_some_and(|v| !valid(v)) {
        return error(StatusCode::BAD_REQUEST);
    }
    let scope = match update.scope.as_str() {
        "user" => PreferenceScope::User,
        "device" => PreferenceScope::Device,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let value = match update.value.map(|v| serde_json::to_string(&v)).transpose() {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let Some(repo) = state.workspace.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || {
        repo.save_workspace(owner, device, update.revision, scope, value)
    })
    .await
    {
        Ok(Ok(v)) => response(owner, v),
        Ok(Err(e)) => storage_error(e),
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
