//! Authenticated presentation settings. Browser profile IDs never authenticate users.
use crate::{
    GatewayState,
    server::{authenticate, credential, secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use rumahl_core::{
    BrowserProfileId, PreferenceScope, ShellMode, ShellPreferences, ShellPreferencesError, UserId,
};
use std::{collections::HashMap, sync::Arc};

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/preferences", get(load).put(save))
        .layer(DefaultBodyLimit::max(1024))
}
fn response(user: UserId, preferences: ShellPreferences) -> Response {
    let mut response = axum::Json(serde_json::json!({"settingsVersion":1,"ownerId":user.to_string(),"revision":preferences.revision,"user":{"shellMode":preferences.user_mode.as_str()},"device":{"shellMode":preferences.device_mode.map(ShellMode::as_str)},"effective":{"shellMode":preferences.effective_mode().as_str()}})).into_response();
    secure_headers(&mut response);
    response
}
fn error(status: StatusCode) -> Response {
    let mut response = status.into_response();
    secure_headers(&mut response);
    response
}
fn storage_error(error_value: ShellPreferencesError) -> Response {
    error(match error_value {
        ShellPreferencesError::Conflict => StatusCode::CONFLICT,
        ShellPreferencesError::Limit => StatusCode::TOO_MANY_REQUESTS,
        ShellPreferencesError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}
fn profile(query: &HashMap<String, String>) -> Option<BrowserProfileId> {
    if query.len() != 1 {
        return None;
    }
    BrowserProfileId::parse(query.get("device")?)
}
async fn user(state: &Arc<GatewayState>, headers: &HeaderMap) -> Result<UserId, Response> {
    let token = credential(headers).map_err(|_| error(StatusCode::UNAUTHORIZED))?;
    authenticate(state, &token)
        .await
        .map(|identity| identity.user_id)
        .map_err(|value| {
            error(match value {
                crate::ShellBackendError::Unauthorized => StatusCode::UNAUTHORIZED,
                crate::ShellBackendError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            })
        })
}
async fn load(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let user = match user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(device) = profile(&query) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repository) = state.preferences.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repository.load(user, device)).await {
        Ok(Ok(value)) => response(user, value),
        Ok(Err(value)) => storage_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
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
    let user = match user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(device) = profile(&query) else {
        return error(StatusCode::BAD_REQUEST);
    };
    if headers.get("content-type").and_then(|h| h.to_str().ok()) != Some("application/json") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object.len() != 5 || value["settingsVersion"] != 1 || value["key"] != "shell.mode" {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(revision) = value["revision"]
        .as_u64()
        .filter(|value| *value <= 9_007_199_254_740_991)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let scope = match value["scope"].as_str() {
        Some("user") => PreferenceScope::User,
        Some("device") => PreferenceScope::Device,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let mode = if value["value"].is_null() && object.contains_key("value") {
        None
    } else if let Some(mode) = value["value"].as_str().and_then(ShellMode::parse) {
        Some(mode)
    } else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repository) = state.preferences.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repository.save(user, device, revision, scope, mode))
        .await
    {
        Ok(Ok(value)) => response(user, value),
        Ok(Err(value)) => storage_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
