use crate::{
    GatewayState,
    preferences::{error, profile, storage_error, user},
    server::secure_headers,
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
    #[serde(default)]
    fullscreen_chrome: FullscreenChrome,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FullscreenChrome {
    #[serde(default = "default_true")]
    hide_topbar: bool,
    #[serde(default = "default_true")]
    hide_dock: bool,
    /// Keeps window bottoms above a visible dock; the shell always sends it.
    #[serde(default)]
    dock_clearance: bool,
}
impl Default for FullscreenChrome {
    fn default() -> Self {
        Self {
            hide_topbar: true,
            hide_dock: true,
            dock_clearance: false,
        }
    }
}
fn default_true() -> bool {
    true
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
    glass_color: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glass_reduced: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mobile_nav_svg_glass: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    animations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    performance_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ultra_performance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adaptive_type: Option<bool>,
    #[serde(default)]
    seed: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}
fn valid_appearance(appearance: &Appearance) -> bool {
    appearance.seed.as_deref().is_none_or(|seed| {
        seed.len() == 7 && seed.starts_with('#') && seed[1..].bytes().all(|b| b.is_ascii_hexdigit())
    }) && appearance
        .mode
        .as_deref()
        .is_none_or(|mode| ["light", "dark"].contains(&mode))
        && appearance
            .glass_engine
            .as_deref()
            .is_none_or(|engine| ["css", "canvas"].contains(&engine))
        && appearance.glass_tint.as_deref().is_none_or(|tint| {
            tint.len() == 7
                && tint.starts_with('#')
                && tint[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        && appearance
            .glass_backend
            .as_deref()
            .is_none_or(|backend| ["auto", "svg", "webgl", "css"].contains(&backend))
        && appearance
            .glass_quality
            .as_deref()
            .is_none_or(|quality| ["auto", "high", "balanced", "low"].contains(&quality))
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
            .is_none_or(|view| ["grid", "deck", "canvas"].contains(&view))
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
    if !state.config.accepts_origin(&headers) {
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
        let old: Appearance =
            serde_json::from_str(r#"{"seed":null,"mode":null,"tokens":{}}"#).unwrap();
        assert!(!old.auto_color);
        let new: Appearance = serde_json::from_str(r#"{"autoColor":true,"tokens":{}}"#).unwrap();
        assert!(valid_appearance(&new));
        let stored = serde_json::to_value(&new).unwrap();
        assert_eq!(stored["autoColor"], true);
    }

    #[test]
    fn fullscreen_chrome_defaults_on_and_roundtrips() {
        let old: Workspace = serde_json::from_str(
            r#"{"version":1,"windows":[],"folders":[],"appearance":{"tokens":{}}}"#,
        )
        .unwrap();
        assert!(old.fullscreen_chrome.hide_topbar);
        assert!(old.fullscreen_chrome.hide_dock);

        let custom: Workspace = serde_json::from_str(
            r#"{"version":1,"windows":[],"folders":[],"appearance":{"tokens":{}},"fullscreenChrome":{"hideTopbar":false,"hideDock":true}}"#,
        )
        .unwrap();
        assert!(!custom.fullscreen_chrome.hide_topbar);
        assert!(custom.fullscreen_chrome.hide_dock);
        let stored = serde_json::to_value(&custom).unwrap();
        assert_eq!(stored["fullscreenChrome"]["hideTopbar"], false);
    }

    /// The shell always sends `dockClearance` with the chrome settings, so the
    /// stored shape must accept it; anything else stays rejected.
    #[test]
    fn fullscreen_chrome_accepts_the_shell_payload() {
        let value: Workspace = serde_json::from_str(
            r#"{"version":1,"windows":[],"folders":[],"fullscreenChrome":{"hideTopbar":true,"hideDock":false,"dockClearance":true}}"#,
        )
        .unwrap();
        assert!(value.fullscreen_chrome.hide_topbar);
        assert!(!value.fullscreen_chrome.hide_dock);
        assert!(value.fullscreen_chrome.dock_clearance);
        assert!(valid(&value));
        assert_eq!(
            serde_json::to_value(&value).unwrap()["fullscreenChrome"]["dockClearance"],
            true
        );
        assert!(
            serde_json::from_str::<Workspace>(
                r#"{"version":1,"windows":[],"folders":[],"fullscreenChrome":{"hideTopbar":true,"hideDock":true,"unknown":true}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn mobile_nav_svg_glass_roundtrips() {
        let value: Appearance =
            serde_json::from_str(r#"{"mobileNavSvgGlass":true,"tokens":{}}"#).unwrap();
        assert_eq!(value.mobile_nav_svg_glass, Some(true));
        assert!(valid_appearance(&value));
        let stored = serde_json::to_value(&value).unwrap();
        assert_eq!(stored["mobileNavSvgGlass"], true);
    }

    #[test]
    fn glass_colour_roundtrips_and_rejects_invalid_values() {
        let valid: Appearance =
            serde_json::from_str(r##"{"glassColor":false,"glassTint":"#ff5500","tokens":{}}"##)
                .unwrap();
        assert!(valid_appearance(&valid));
        assert_eq!(
            serde_json::to_value(&valid).unwrap()["glassTint"],
            "#ff5500"
        );
        let invalid: Appearance =
            serde_json::from_str(r##"{"glassTint":"red; display:none","tokens":{}}"##).unwrap();
        assert!(!valid_appearance(&invalid));
    }
}
