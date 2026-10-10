//! Exposure-mode policy endpoint.
//!
//! Reading the effective mode is like any authenticated shell request. Raising
//! the mode to `advanced` or `developer` is security-sensitive and requires the
//! account password (re-authentication); lowering it or removing an override
//! does not. The JSON body is an allow-listed, bounded field set.
use crate::{
    GatewayState, ShellBackendError,
    preferences::{error, profile, user},
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
use rumahl_core::{OsMode, OsModeSettings, OsModeStoreError, PreferenceScope, UserId};
use std::{collections::HashMap, sync::Arc};
use zeroize::Zeroizing;

const MAX_PASSWORD_BYTES: usize = 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/os-mode", get(load).put(save))
        .layer(DefaultBodyLimit::max(4096))
}

fn response(user: UserId, settings: OsModeSettings) -> Response {
    let OsModeSettings {
        revision,
        user_mode,
        device_mode,
    } = settings;
    let effective = device_mode.unwrap_or(user_mode);
    let mut response = axum::Json(serde_json::json!({
        "settingsVersion": 1,
        "ownerId": user.to_string(),
        "revision": revision,
        "user": { "osMode": user_mode.as_str() },
        "device": { "osMode": device_mode.map(|mode| mode.as_str()) },
        "effective": { "osMode": effective.as_str() },
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

fn store_error(value: OsModeStoreError) -> Response {
    error(match value {
        OsModeStoreError::Conflict => StatusCode::CONFLICT,
        OsModeStoreError::Limit => StatusCode::TOO_MANY_REQUESTS,
        OsModeStoreError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
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
    let Some(repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repository.load(user, device)).await {
        Ok(Ok(settings)) => response(user, settings),
        Ok(Err(value)) => store_error(value),
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
    // Allow-list the fields so an unexpected key can never be interpreted.
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "settingsVersion" | "revision" | "scope" | "mode" | "password"
        )
    }) {
        return error(StatusCode::BAD_REQUEST);
    }
    if value["settingsVersion"] != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(revision) = value["revision"]
        .as_u64()
        .filter(|value| *value <= MAX_SAFE_INTEGER)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let scope = match value["scope"].as_str() {
        Some("user") => PreferenceScope::User,
        Some("device") => PreferenceScope::Device,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let mode = if value["mode"].is_null() && object.contains_key("mode") {
        None
    } else if let Some(mode) = value["mode"]
        .as_str()
        .and_then(|raw| OsMode::parse(raw).ok())
    {
        Some(mode)
    } else {
        return error(StatusCode::BAD_REQUEST);
    };

    // Raising the mode requires re-authentication; anything else must not carry a password.
    let raising = matches!(mode, Some(OsMode::Advanced) | Some(OsMode::Developer));
    let has_password = object.contains_key("password");
    if raising != has_password {
        return error(StatusCode::BAD_REQUEST);
    }
    if raising {
        let Some(sessions) = state.browser_sessions.clone() else {
            return error(StatusCode::SERVICE_UNAVAILABLE);
        };
        let Some(password) = value["password"].as_str() else {
            return error(StatusCode::BAD_REQUEST);
        };
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return error(StatusCode::BAD_REQUEST);
        }
        let mut password = Zeroizing::new(password.to_owned());
        let verified = tokio::task::spawn_blocking(move || {
            sessions.reauthenticate(user, std::mem::take(&mut *password))
        })
        .await;
        match verified {
            Ok(Ok(())) => {}
            Ok(Err(ShellBackendError::Unauthorized)) => return error(StatusCode::FORBIDDEN),
            _ => return error(StatusCode::SERVICE_UNAVAILABLE),
        }
    }

    let Some(repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repository.save(user, device, revision, scope, mode))
        .await
    {
        Ok(Ok(settings)) => response(user, settings),
        Ok(Err(value)) => store_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
