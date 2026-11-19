//! Read-only host filesystem browsing for the file explorer.
//!
//! The explorer can browse the managed personal store (the `files` API) and, in
//! advanced mode or above, the real host roots (`/apps`, `/rumahl`). Paths are
//! always relative to a named area root and validated by the provider, never
//! raw absolute host paths from the client.
use crate::{
    GatewayState,
    preferences::{error, user},
    server::secure_headers,
};
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use rumahl_core::BrowserProfileId;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    pub name: String,
    pub directory: bool,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArea {
    Apps,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    Missing,
    Invalid,
    Denied,
    Unavailable,
    Limit,
}

/// Persistence boundary for the real host roots the explorer may browse.
pub trait HostFiles: Send + Sync + 'static {
    fn list(&self, area: HostArea, path: &str) -> Result<Vec<HostEntry>, HostError>;
    fn read(&self, area: HostArea, path: &str) -> Result<(String, Vec<u8>), HostError>;
}

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/fs", get(list))
        .route("/api/v1/shell/fs/content", get(download))
}

fn parse_area(value: &str) -> Option<HostArea> {
    match value {
        "apps" => Some(HostArea::Apps),
        "system" => Some(HostArea::System),
        _ => None,
    }
}

fn fail(e: HostError) -> Response {
    error(match e {
        HostError::Missing => StatusCode::NOT_FOUND,
        HostError::Invalid => StatusCode::BAD_REQUEST,
        HostError::Denied => StatusCode::FORBIDDEN,
        HostError::Limit => StatusCode::PAYLOAD_TOO_LARGE,
        HostError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}

/// Resolves the requested area and path and enforces the OS-mode gate: real
/// host roots are only visible from advanced mode upwards.
async fn context(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
    query: &HashMap<String, String>,
) -> Result<(HostArea, String, Arc<dyn HostFiles>), Response> {
    let user = user(state, headers).await?;
    if query.len() < 2 || query.len() > 3 {
        return Err(error(StatusCode::BAD_REQUEST));
    }
    let Some(area) = query.get("area").and_then(|value| parse_area(value)) else {
        return Err(error(StatusCode::BAD_REQUEST));
    };
    let Some(device) = query
        .get("device")
        .and_then(|value| BrowserProfileId::parse(value))
    else {
        return Err(error(StatusCode::BAD_REQUEST));
    };
    let path = query.get("path").cloned().unwrap_or_default();
    let Some(mode_repository) = state.os_mode.clone() else {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE));
    };
    let mode = match tokio::task::spawn_blocking(move || mode_repository.load(user, device)).await {
        Ok(Ok(settings)) => settings.effective_mode(),
        Ok(Err(_)) => return Err(error(StatusCode::SERVICE_UNAVAILABLE)),
        Err(_) => return Err(error(StatusCode::SERVICE_UNAVAILABLE)),
    };
    if !mode.policy().can_browse_system_files() {
        return Err(error(StatusCode::FORBIDDEN));
    }
    let Some(provider) = state.host_files.clone() else {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE));
    };
    Ok((area, path, provider))
}

async fn list(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let (area, path, provider) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let requested = path.clone();
    match tokio::task::spawn_blocking(move || provider.list(area, &requested)).await {
        Ok(Ok(entries)) => {
            let entries: Vec<_> = entries
                .into_iter()
                .map(|entry| {
                    serde_json::json!({
                        "name": entry.name,
                        "directory": entry.directory,
                        "size": entry.size,
                        "modified": entry.modified,
                    })
                })
                .collect();
            let mut response = axum::Json(serde_json::json!({
                "fsVersion": 1,
                "area": match area { HostArea::Apps => "apps", HostArea::System => "system" },
                "path": path,
                "entries": entries,
            }))
            .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn download(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let (area, path, provider) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || provider.read(area, &path)).await {
        Ok(Ok((name, data))) => {
            let encoded = name
                .bytes()
                .map(|byte| format!("%{byte:02X}"))
                .collect::<String>();
            let mut response = data.into_response();
            secure_headers(&mut response);
            response
                .headers_mut()
                .insert("content-type", "application/octet-stream".parse().unwrap());
            response.headers_mut().insert(
                "content-disposition",
                format!("attachment; filename=\"download\"; filename*=UTF-8''{encoded}")
                    .parse()
                    .unwrap(),
            );
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
