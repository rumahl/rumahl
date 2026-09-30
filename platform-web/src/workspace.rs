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
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Workspace {
    version: u8,
    windows: Vec<Window>,
    folders: Vec<Folder>,
    #[serde(default)]
    desktop: Desktop,
    #[serde(default)]
    launcher_view: Option<String>,
    #[serde(default)]
    appearance: Appearance,
}
#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Appearance {
    #[serde(default)]
    auto_color: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wallpaper_tint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wallpaper_motion: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_reduced: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    animations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    performance_mode: Option<bool>,
    #[serde(default)]
    seed: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}
fn valid_appearance(appearance: &Appearance) -> bool {
    appearance
        .seed
        .as_deref()
        .map_or(true, |seed| {
            seed.len() == 7
                && seed.starts_with('#')
                && seed[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
        && appearance
            .mode
            .as_deref()
            .map_or(true, |mode| ["light", "dark"].contains(&mode))
        && appearance
            .glass_engine
            .as_deref()
            .map_or(true, |engine| ["css", "canvas"].contains(&engine))
        && appearance
            .glass_backend
            .as_deref()
            .map_or(true, |backend| ["auto", "svg", "webgl", "css"].contains(&backend))
        && appearance
            .glass_quality
            .as_deref()
            .map_or(true, |quality| ["auto", "high", "balanced", "low"].contains(&quality))
        && appearance.tokens.len() <= 64
        && appearance.tokens.iter().all(|(key, value)| {
            !key.is_empty()
                && key.len() <= 256
                && !value.is_empty()
                && value.len() <= 256
                && !value.contains([';', '{', '}', '\\', '<', '>', '@'])
        })
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Desktop {
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    hidden: Vec<String>,
    #[serde(default = "default_widgets")]
    widgets: bool,
}
impl Default for Desktop {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            hidden: Vec::new(),
            widgets: true,
        }
    }
}
fn default_widgets() -> bool {
    true
}
fn valid_arrangement(desktop: &Desktop) -> bool {
    [&desktop.order, &desktop.hidden].iter().all(|list| {
        list.len() <= 512
            && list.iter().all(|id| {
                !id.is_empty()
                    && id.len() <= 255
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            })
    })
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
        && valid_arrangement(&value.desktop)
        && valid_appearance(&value.appearance)
        && value
            .launcher_view
            .as_deref()
            .map_or(true, |view| ["grid", "deck", "canvas"].contains(&view))
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

#[cfg(test)]
mod appearance_tests {
    use super::*;

    #[test]
    fn automatic_colour_roundtrips_and_defaults_off() {
        let old: Appearance = serde_json::from_str(r#"{"seed":null,"mode":null,"tokens":{}}"#).unwrap();
        assert!(!old.auto_color);
        let new: Appearance = serde_json::from_str(r#"{"autoColor":true,"tokens":{}}"#).unwrap();
        assert!(valid_appearance(&new));
        let stored = serde_json::to_value(&new).unwrap();
        assert_eq!(stored["autoColor"], true);
    }
}
