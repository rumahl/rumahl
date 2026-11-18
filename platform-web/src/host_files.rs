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
    server::{record_audit, secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use rumahl_core::{AuditAction, AuditOutcome, BrowserProfileId, UserId};
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
    /// Returns an entry's own name and whether it is a directory, without
    /// reading its content or listing a whole folder.
    fn stat(&self, area: HostArea, path: &str, user: UserId) -> Result<(String, bool), HostError>;
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
        .route("/api/v1/shell/fs/resolve", get(resolve_path))
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

/// One parsed `rumahl://` path. Desktop is virtual; the other areas map to a
/// real host root and a path relative to it.
#[derive(Debug, PartialEq, Eq)]
enum RumahlTarget {
    Desktop,
    Host { area: HostArea, relative: String },
}

/// Parses the canonical rumahl path syntax: `rumahl://home/Documents`,
/// `rumahl://apps/<app-id>/...`, `rumahl://system/...`, `rumahl://desktop`.
///
/// The syntax is deliberately not a POSIX path: it has no drive letters, no
/// absolute host paths, and `..` never escapes an area. Client-side input
/// normalization (aliases, backslashes) happens before this parser runs.
fn parse_rumahl_path(value: &str) -> Option<RumahlTarget> {
    const MAX_PATH_BYTES: usize = 4096;
    const MAX_NAME_BYTES: usize = 255;
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return None;
    }
    let (scheme, rest) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("rumahl") {
        return None;
    }
    let mut parts = rest
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".");
    let area = parts.next()?.to_ascii_lowercase();
    let segments: Vec<&str> = parts.collect();
    if segments
        .iter()
        .any(|segment| *segment == ".." || segment.len() > MAX_NAME_BYTES)
    {
        return None;
    }
    let relative = segments.join("/");
    let target = match area.as_str() {
        "home" => HostArea::Home,
        "apps" => HostArea::Apps,
        "system" => HostArea::System,
        "desktop" if relative.is_empty() => return Some(RumahlTarget::Desktop),
        _ => return None,
    };
    Some(RumahlTarget::Host {
        area: target,
        relative,
    })
}

/// The canonical form shown in the address bar.
fn rumahl_path(area: HostArea, relative: &str) -> String {
    if relative.is_empty() {
        format!("rumahl://{}", area_name(area))
    } else {
        format!("rumahl://{}/{relative}", area_name(area))
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
    authorize_area(state, user, device, area).await?;
    let Some(provider) = state.host_files.clone() else {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE));
    };
    Ok((area, path, provider, user))
}

/// The user's own home is their data, not a system surface: it needs no
/// exposure mode. Installed apps and the system tree stay advanced-only.
async fn authorize_area(
    state: &Arc<GatewayState>,
    user: UserId,
    device: BrowserProfileId,
    area: HostArea,
) -> Result<(), Response> {
    if area == HostArea::Home {
        return Ok(());
    }
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
    Ok(())
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

/// Resolves a typed `rumahl://` path for the address bar: it decides whether
/// the target is a folder or a file, whether it exists, and whether the caller
/// may see that area. The frontend never interprets host paths itself.
async fn resolve_path(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let user = match user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if query.len() != 2 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(device) = query
        .get("device")
        .and_then(|value| BrowserProfileId::parse(value))
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(raw) = query.get("path") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let target = match parse_rumahl_path(raw) {
        Some(target) => target,
        None => return error(StatusCode::BAD_REQUEST),
    };
    let (path, area, relative, name, directory, writable) = match target {
        RumahlTarget::Desktop => (
            "rumahl://desktop".to_owned(),
            "desktop",
            String::new(),
            "Desktop".to_owned(),
            true,
            true,
        ),
        RumahlTarget::Host { area, relative } => {
            if let Err(response) = authorize_area(&state, user, device, area).await {
                return response;
            }
            let Some(provider) = state.host_files.clone() else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            };
            let canonical = rumahl_path(area, &relative);
            let requested = relative.clone();
            match tokio::task::spawn_blocking(move || provider.stat(area, &requested, user)).await {
                Ok(Ok((name, directory))) => (
                    canonical,
                    area_name(area),
                    relative,
                    name,
                    directory,
                    area == HostArea::Home,
                ),
                Ok(Err(failure)) => return fail(failure),
                Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
            }
        }
    };
    let mut response = axum::Json(serde_json::json!({
        "fsVersion": 1,
        "path": path,
        "area": area,
        "relative": relative,
        "name": name,
        "directory": directory,
        "writable": writable,
    }))
    .into_response();
    secure_headers(&mut response);
    response
}

/// Records one home mutation, success or failure, in the central audit log.
fn audit_change(
    state: &GatewayState,
    user: UserId,
    area: HostArea,
    path: &str,
    outcome: AuditOutcome,
) {
    let target = format!("{}:{path}", area_name(area));
    record_audit(
        state,
        rumahl_core::AuditActor::User(user),
        AuditAction::HostFileChanged,
        outcome,
        Some(&target),
    );
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
    let audited = path.clone();
    match tokio::task::spawn_blocking(move || provider.write(area, &path, user, &data)).await {
        Ok(Ok(())) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Success);
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Failure);
            fail(error)
        }
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
    let audited = if parent.is_empty() {
        name.clone()
    } else {
        format!("{parent}/{name}")
    };
    match tokio::task::spawn_blocking(move || {
        provider.create(area, &parent, &name, directory, user)
    })
    .await
    {
        Ok(Ok(())) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Success);
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Failure);
            fail(error)
        }
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
    let audited = path.clone();
    match tokio::task::spawn_blocking(move || provider.rename(area, &path, &name, user)).await {
        Ok(Ok(())) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Success);
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Failure);
            fail(error)
        }
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
    let audited = path.clone();
    match tokio::task::spawn_blocking(move || provider.remove(area, &path, user)).await {
        Ok(Ok(())) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Success);
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(error)) => {
            audit_change(&state, user, area, &audited, AuditOutcome::Failure);
            fail(error)
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_canonical_rumahl_paths() {
        assert_eq!(
            parse_rumahl_path("rumahl://home"),
            Some(RumahlTarget::Host {
                area: HostArea::Home,
                relative: String::new()
            })
        );
        assert_eq!(
            parse_rumahl_path("rumahl://home/Documents/notes.txt"),
            Some(RumahlTarget::Host {
                area: HostArea::Home,
                relative: "Documents/notes.txt".to_owned()
            })
        );
        // The scheme and the area are case-insensitive; the path is not.
        assert_eq!(
            parse_rumahl_path("RUMAHL://Home/Documents"),
            Some(RumahlTarget::Host {
                area: HostArea::Home,
                relative: "Documents".to_owned()
            })
        );
        // Empty and "." segments are normalized, never interpreted.
        assert_eq!(
            parse_rumahl_path("rumahl://home//Documents/./x/"),
            Some(RumahlTarget::Host {
                area: HostArea::Home,
                relative: "Documents/x".to_owned()
            })
        );
        assert_eq!(
            parse_rumahl_path("rumahl://desktop"),
            Some(RumahlTarget::Desktop)
        );
        for invalid in [
            "rumahl://desktop/Documents",
            "rumahl://home/../etc",
            "rumahl://home/Documents/..",
            "rumahl://home\\Documents",
            "rumahl://home/Documents\0",
            "/home/alice",
            "C:\\Users\\alice",
            "home/Documents",
            "rumahl://root/etc",
            "rumahl://",
            "https://example.test/home",
        ] {
            assert_eq!(parse_rumahl_path(invalid), None, "{invalid}");
        }
    }
}
