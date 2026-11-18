//! Host filesystem browsing and the writable home area for the file explorer.
//!
//! The explorer can browse the managed personal store (the `files` API) and, in
//! advanced mode or above, the real host roots: `/apps`, the protected
//! `/rumahl` tree and the calling user's own home. Paths are always relative to
//! a named area root and validated by the provider, never raw absolute host
//! paths from the client. Only the home area is writable; installed apps and
//! the system tree stay read-only for the shell.
use crate::{
    GatewayState,
    preferences::{error, user},
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
use rumahl_core::{BrowserProfileId, UserId};
use std::{collections::HashMap, sync::Arc};

/// One entry of a host directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    pub name: String,
    pub directory: bool,
    pub size: u64,
    pub modified: u64,
}

/// Named roots the explorer addresses. `Home` is the calling user's own home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArea {
    Apps,
    System,
    Home,
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
///
/// The calling user is always part of the request: `Home` resolves to that
/// user's home directory, and no area ever inherits another user's path.
/// Writes are only honored for `Home`; implementations must refuse the other
/// areas.
pub trait HostFiles: Send + Sync + 'static {
    fn list(&self, area: HostArea, path: &str, user: UserId) -> Result<Vec<HostEntry>, HostError>;
    fn read(
        &self,
        area: HostArea,
        path: &str,
        user: UserId,
    ) -> Result<(String, Vec<u8>), HostError>;
    /// Replaces or creates one file inside the user's home.
    fn write(&self, area: HostArea, path: &str, user: UserId, data: &[u8])
    -> Result<(), HostError>;
    /// Creates a file or directory named `name` inside `parent`.
    fn create(
        &self,
        area: HostArea,
        parent: &str,
        name: &str,
        directory: bool,
        user: UserId,
    ) -> Result<(), HostError>;
    /// Removes a file or an empty directory from the user's home.
    fn remove(&self, area: HostArea, path: &str, user: UserId) -> Result<(), HostError>;
    /// Renames one entry inside the user's home.
    fn rename(&self, area: HostArea, path: &str, name: &str, user: UserId)
    -> Result<(), HostError>;
}

/// Mirrors the personal-files body limit; a host write is a user document too.
pub(crate) const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 255;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route(
            "/api/v1/shell/fs",
            get(list).post(create).put(rename).delete(remove),
        )
        .route(
            "/api/v1/shell/fs/content",
            get(download)
                .put(write)
                .layer(DefaultBodyLimit::max(MAX_WRITE_BYTES)),
        )
}

fn parse_area(value: &str) -> Option<HostArea> {
    match value {
        "apps" => Some(HostArea::Apps),
        "system" => Some(HostArea::System),
        "home" => Some(HostArea::Home),
        _ => None,
    }
}

pub(crate) fn area_name(area: HostArea) -> &'static str {
    match area {
        HostArea::Apps => "apps",
        HostArea::System => "system",
        HostArea::Home => "home",
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
#[allow(clippy::type_complexity)]
async fn context(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
    query: &HashMap<String, String>,
) -> Result<(HostArea, String, Arc<dyn HostFiles>, UserId), Response> {
    let user = user(state, headers).await?;
    // list/read: area, path, device; rename adds name; create adds name and
    // directory. Anything else is refused before the provider is consulted.
    if query.len() < 2 || query.len() > 5 {
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
    Ok((area, path, provider, user))
}

/// Mutating requests need the origin check, exactly like every other write in
/// the shell; reads stay plain authenticated requests.
fn allow_mutation(state: &GatewayState, headers: &HeaderMap) -> Option<Response> {
    if valid_origin(headers, &state.config.public_origin) {
        None
    } else {
        Some(error(StatusCode::FORBIDDEN))
    }
}

#[allow(clippy::result_large_err)]
fn name_of(query: &HashMap<String, String>) -> Result<String, Response> {
    let Some(name) = query.get("name") else {
        return Err(error(StatusCode::BAD_REQUEST));
    };
    if name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err(error(StatusCode::BAD_REQUEST));
    }
    Ok(name.clone())
}

async fn list(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let (area, path, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let requested = path.clone();
    match tokio::task::spawn_blocking(move || provider.list(area, &requested, user)).await {
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
                "area": area_name(area),
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
    let (area, path, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || provider.read(area, &path, user)).await {
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

async fn write(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = allow_mutation(&state, &headers) {
        return response;
    }
    let (area, path, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if area != HostArea::Home {
        return error(StatusCode::FORBIDDEN);
    }
    if body.len() > MAX_WRITE_BYTES {
        return error(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let data = body.to_vec();
    match tokio::task::spawn_blocking(move || provider.write(area, &path, user, &data)).await {
        Ok(Ok(())) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn create(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Some(response) = allow_mutation(&state, &headers) {
        return response;
    }
    let (area, parent, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if area != HostArea::Home {
        return error(StatusCode::FORBIDDEN);
    }
    let name = match name_of(&query) {
        Ok(name) => name,
        Err(response) => return response,
    };
    let directory = match query.get("directory").map(String::as_str) {
        Some("true") => true,
        Some("false") => false,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    match tokio::task::spawn_blocking(move || {
        provider.create(area, &parent, &name, directory, user)
    })
    .await
    {
        Ok(Ok(())) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn rename(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Some(response) = allow_mutation(&state, &headers) {
        return response;
    }
    let (area, path, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if area != HostArea::Home {
        return error(StatusCode::FORBIDDEN);
    }
    let name = match name_of(&query) {
        Ok(name) => name,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || provider.rename(area, &path, &name, user)).await {
        Ok(Ok(())) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn remove(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Some(response) = allow_mutation(&state, &headers) {
        return response;
    }
    let (area, path, provider, user) = match context(&state, &headers, &query).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if area != HostArea::Home {
        return error(StatusCode::FORBIDDEN);
    }
    match tokio::task::spawn_blocking(move || provider.remove(area, &path, user)).await {
        Ok(Ok(())) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => fail(error),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
