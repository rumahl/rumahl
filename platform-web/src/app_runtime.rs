//! Per-app runtime environment: the recorded view of an installed image.
//!
//! The sibling of the platform configuration view. It is gated by the same
//! advanced-mode policy and shows the same class of information — what a
//! container image brought with itself, captured at import time by the
//! runtime supervisor and recorded by the service. Names that could carry a
//! credential were already filtered before the record reached the provider.

use crate::{
    AppAccessError, GatewayState, ShellBackendError, ShellIdentity,
    server::{authenticate, credential, error, secure_headers},
};
use axum::{
    Router,
    extract::{Path as UrlPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{collections::HashMap, sync::Arc};

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    // Distinct from `/runtime`, which is the runtime control state: this is
    // the recorded image view.
    Router::new().route("/api/v1/shell/apps/{id}/image", get(load))
}

#[allow(clippy::result_large_err)]
async fn identity_of(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
) -> Result<ShellIdentity, Response> {
    let token = credential(headers).map_err(|_| error(StatusCode::UNAUTHORIZED))?;
    authenticate(state, &token).await.map_err(|value| {
        error(match value {
            ShellBackendError::Unauthorized => StatusCode::UNAUTHORIZED,
            ShellBackendError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        })
    })
}

async fn load(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if query.len() != 2 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(installation) = query.get("installationId") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(device) = query
        .get("device")
        .and_then(|value| rumahl_core::BrowserProfileId::parse(value))
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    // Same gate as the platform configuration: an advanced surface.
    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mode =
        match tokio::task::spawn_blocking(move || mode_repository.load(identity.user_id, device))
            .await
        {
            Ok(Ok(settings)) => settings.effective_mode(),
            Ok(Err(_)) => return error(StatusCode::SERVICE_UNAVAILABLE),
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
        };
    if !mode.policy().can_view_platform_config() {
        return error(StatusCode::FORBIDDEN);
    }
    let Some(apps) = state.apps.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let installation_for_call = installation.clone();
    let id_for_call = id.clone();
    match tokio::task::spawn_blocking(move || {
        apps.app_runtime_environment(identity, &id_for_call, &installation_for_call)
    })
    .await
    {
        Ok(Ok(environment)) => {
            let mut response = axum::Json(serde_json::json!({
                "id": id,
                "installationId": installation,
                "runtime": environment.runtime,
                "image": environment.image,
                "entries": environment
                    .entries
                    .iter()
                    .map(|entry| serde_json::json!({
                        "name": entry.name,
                        "value": entry.value,
                    }))
                    .collect::<Vec<_>>(),
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(AppAccessError::Denied)) => {
            eprintln!("[dbg] denied");
            error(StatusCode::NOT_FOUND)
        }
        Ok(Err(AppAccessError::Unavailable)) => {
            eprintln!("[dbg] unavailable");
            error(StatusCode::SERVICE_UNAVAILABLE)
        }
        Err(join) => {
            eprintln!("[dbg] join error: {join:?}");
            error(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}
