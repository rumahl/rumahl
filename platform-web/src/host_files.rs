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
    /// The caller's account name, used to validate the `/home/<name>` form of
    /// an internal path. It is never another user's name.
    fn username(&self, user: UserId) -> Result<String, HostError>;
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

/// One parsed path. Desktop is virtual; home carries the optional account
/// name from an internal absolute path; apps/system map to a real host root.
#[derive(Debug, PartialEq, Eq)]
enum PathTarget {
    Desktop,
    Home {
        username: Option<String>,
        relative: String,
    },
    Host {
        area: HostArea,
        relative: String,
    },
}

/// Parses both address forms into one target:
///
/// * `rumahl://home/Documents`, `rumahl://apps/<app-id>/...`,
///   `rumahl://system/...`, `rumahl://desktop` — the client-neutral form.
/// * `/home/<username>/Documents`, `/apps/...`, `/system/...` — the internal
///   absolute form that other OS parts and apps resolve.
///
/// Neither form can reach another user's home: the account name in
/// `/home/<name>` must be the caller's, and `..` never escapes an area. There
/// are no drive letters and no arbitrary host paths.
fn parse_path(value: &str) -> Option<PathTarget> {
    const MAX_PATH_BYTES: usize = 4096;
    const MAX_NAME_BYTES: usize = 255;
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return None;
    }
    if let Some(rest) = value.strip_prefix('/') {
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
        return match area.as_str() {
            "home" => {
                let (username, rest) = relative.split_once('/').unwrap_or((&relative, ""));
                if username.is_empty() || username == "." || username == ".." {
                    return None;
                }
                Some(PathTarget::Home {
                    username: Some(username.to_owned()),
                    relative: rest.to_owned(),
                })
            }
            "apps" => Some(PathTarget::Host {
                area: HostArea::Apps,
                relative,
            }),
            "system" => Some(PathTarget::Host {
                area: HostArea::System,
                relative,
            }),
            _ => None,
        };
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
    match area.as_str() {
        "home" => Some(PathTarget::Home {
            username: None,
            relative,
        }),
        "apps" => Some(PathTarget::Host {
            area: HostArea::Apps,
            relative,
        }),
        "system" => Some(PathTarget::Host {
            area: HostArea::System,
            relative,
        }),
        "desktop" if relative.is_empty() => Some(PathTarget::Desktop),
        _ => None,
    }
}

/// The client-neutral form shown in the address bar.
fn rumahl_path(area: HostArea, relative: &str) -> String {
    if relative.is_empty() {
        format!("rumahl://{}", area_name(area))
    } else {
        format!("rumahl://{}/{relative}", area_name(area))
    }
}

/// The internal absolute form other OS parts resolve.
fn internal_path(username: &str, relative: &str) -> String {
    if relative.is_empty() {
        format!("/home/{username}")
    } else {
        format!("/home/{username}/{relative}")
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
    let target = match parse_path(raw) {
        Some(target) => target,
        None => return error(StatusCode::BAD_REQUEST),
    };
    let (path, uri, area, relative, name, directory, writable, id, trail) = match target {
        PathTarget::Desktop => (
            "rumahl://desktop".to_owned(),
            "rumahl://desktop".to_owned(),
            "desktop",
            String::new(),
            "Desktop".to_owned(),
            true,
            true,
            None,
            None,
        ),
        PathTarget::Host { area, relative } => {
            if let Err(response) = authorize_area(&state, user, device, area).await {
                return response;
            }
            let Some(provider) = state.host_files.clone() else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            };
            let canonical = rumahl_path(area, &relative);
            let internal = match area {
                HostArea::Apps => format!("/apps/{relative}"),
                HostArea::System => format!("/system/{relative}"),
                HostArea::Home => unreachable!("home is handled above"),
            };
            let requested = relative.clone();
            match tokio::task::spawn_blocking(move || provider.stat(area, &requested, user)).await {
                Ok(Ok((name, directory))) => (
                    internal.trim_end_matches('/').to_owned(),
                    canonical,
                    area_name(area),
                    relative,
                    name,
                    directory,
                    false,
                    None,
                    None,
                ),
                Ok(Err(failure)) => return fail(failure),
                Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
            }
        }
        PathTarget::Home { username, relative } => {
            // The account name in an internal path must be the caller's. It is
            // never accepted from the client without that comparison.
            let Some(provider) = state.host_files.clone() else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            };
            let user_for_name = user;
            let account =
                match tokio::task::spawn_blocking(move || provider.username(user_for_name)).await {
                    Ok(Ok(name)) => name,
                    Ok(Err(failure)) => return fail(failure),
                    Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
                };
            if username.as_deref().is_some_and(|name| name != account) {
                return error(StatusCode::FORBIDDEN);
            }
            let provider = match state.host_files.clone() {
                Some(provider) => provider,
                None => return error(StatusCode::SERVICE_UNAVAILABLE),
            };
            let requested = relative.clone();
            let (name, directory) = match tokio::task::spawn_blocking(move || {
                provider.stat(HostArea::Home, &requested, user)
            })
            .await
            {
                Ok(Ok(entry)) => entry,
                Ok(Err(failure)) => return fail(failure),
                Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
            };
            // The personal store is the same home; its stable ids make the
            // path a store trail the Files app can navigate.
            let (id, trail) = match state.files.clone() {
                Some(files) => {
                    let requested = relative.clone();
                    match tokio::task::spawn_blocking(move || files.path_trail(user, &requested))
                        .await
                    {
                        Ok(Ok(entries)) => {
                            let trail = entries
                                .iter()
                                .map(|entry| {
                                    serde_json::json!({
                                        "id": entry.id,
                                        "name": entry.name,
                                        "directory": entry.directory,
                                    })
                                })
                                .collect::<Vec<_>>();
                            let id = entries
                                .last()
                                .map(|entry| entry.id.clone())
                                .unwrap_or_else(|| "root".to_owned());
                            (Some(id), Some(trail))
                        }
                        Ok(Err(_)) => (None, None),
                        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
                    }
                }
                None => (None, None),
            };
            (
                internal_path(&account, &relative),
                rumahl_path(HostArea::Home, &relative),
                "home",
                relative,
                name,
                directory,
                true,
                id,
                trail,
            )
        }
    };
    let mut response = axum::Json(serde_json::json!({
        "fsVersion": 1,
        "path": path,
        "uri": uri,
        "area": area,
        "relative": relative,
        "name": name,
        "directory": directory,
        "writable": writable,
        "id": id,
        "trail": trail,
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
    fn parses_rumahl_and_internal_paths() {
        let home = |username: Option<&str>, relative: &str| {
            Some(PathTarget::Home {
                username: username.map(str::to_owned),
                relative: relative.to_owned(),
            })
        };
        assert_eq!(parse_path("rumahl://home"), home(None, ""));
        assert_eq!(
            parse_path("rumahl://home/Documents/notes.txt"),
            home(None, "Documents/notes.txt")
        );
        // The scheme and the area are case-insensitive; the path is not.
        assert_eq!(
            parse_path("RUMAHL://Home/Documents"),
            home(None, "Documents")
        );
        // Empty and "." segments are normalized, never interpreted.
        assert_eq!(
            parse_path("rumahl://home//Documents/./x/"),
            home(None, "Documents/x")
        );
        // The internal absolute form names the account explicitly.
        assert_eq!(
            parse_path("/home/alice/Documents"),
            home(Some("alice"), "Documents")
        );
        assert_eq!(parse_path("/home/alice"), home(Some("alice"), ""));
        assert_eq!(
            parse_path("/apps/com.example.notes/data"),
            Some(PathTarget::Host {
                area: HostArea::Apps,
                relative: "com.example.notes/data".to_owned()
            })
        );
        assert_eq!(
            parse_path("/system/etc/hosts"),
            Some(PathTarget::Host {
                area: HostArea::System,
                relative: "etc/hosts".to_owned()
            })
        );
        assert_eq!(parse_path("rumahl://desktop"), Some(PathTarget::Desktop));
        for invalid in [
            "rumahl://desktop/Documents",
            "rumahl://home/../etc",
            "/home/alice/../etc",
            "/home/alice/Documents/..",
            "rumahl://home\\Documents",
            "/home/alice\\Documents",
            "rumahl://home/Documents\0",
            "/home",
            "/home/",
            "/etc/passwd",
            "/apps/../secret",
            "C:\\Users\\alice",
            "home/Documents",
            "rumahl://root/etc",
            "rumahl://",
            "https://example.test/home",
        ] {
            assert_eq!(parse_path(invalid), None, "{invalid}");
        }
    }
}
