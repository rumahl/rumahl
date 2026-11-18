//! App permission consent and consented app file access.
//!
//! `GET/PUT/DELETE /api/v1/shell/apps/{id}/grants` is the user's consent
//! surface: what an installed app's manifest requests and which grants the
//! calling user has issued. Consent is always explicit — it names the exact
//! personal files — and it is either durable (persisted with the platform
//! snapshot, optionally with an expiry) or bound to the current browser
//! session. Issuing a durable `rumahl.files.write` consent additionally
//! requires re-authentication, exactly like raising the OS mode.
//!
//! `GET/PUT /api/v1/shell/apps/{id}/files/content` is the enforcement consumer:
//! every request is authorized per call against the app's durable and session
//! grants before any byte moves. A denial reports why, so the shell can offer
//! consent (`approval-required`) or surface a hard refusal.

use crate::apps::{AppFileAccess, AppFileAction, AppFileOutcome, GrantIssue, GrantValidityChoice};
use crate::{
    AppAccess, AppAccessError, AppFileDeny, GatewayState, ShellBackendError, ShellIdentity,
    server::{authenticate, credential, error, record_audit, secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path as UrlPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use rumahl_core::{AuditAction, AuditOutcome, InstallationId};
use std::{collections::HashMap, sync::Arc};
use zeroize::Zeroizing;

/// Longest accepted consent body, plenty for 64 resource keys plus a password.
const MAX_CONSENT_BYTES: usize = 16 * 1024;
const MAX_PASSWORD_BYTES: usize = 1024;
/// One year, in seconds. Longer consents are policies, not picker decisions.
const MAX_CONSENT_SECONDS: u64 = 31_536_000;
const MAX_RESOURCES: usize = 64;
/// Mirrors the personal-files body limit; app writes are user documents, too.
const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;
const MAX_KEY_BYTES: usize = 255;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route(
            "/api/v1/shell/apps/{id}/grants",
            get(grants)
                .put(issue)
                .layer(DefaultBodyLimit::max(MAX_CONSENT_BYTES)),
        )
        .route("/api/v1/shell/apps/{id}/grants/{grant}", delete(revoke))
        .route(
            "/api/v1/shell/apps/{id}/files/content",
            get(read)
                .put(write)
                .layer(DefaultBodyLimit::max(MAX_WRITE_BYTES)),
        )
}

fn json_response(value: serde_json::Value, status: StatusCode) -> Response {
    let mut response = (status, axum::Json(value)).into_response();
    secure_headers(&mut response);
    response
}

fn denied_outcome(reason: &str) -> Response {
    json_response(
        serde_json::json!({"outcome": "denied", "reason": reason}),
        StatusCode::FORBIDDEN,
    )
}

fn file_denied(reason: AppFileDeny) -> Response {
    match reason {
        AppFileDeny::NoGrant => denied_outcome("no-grant"),
        AppFileDeny::ApprovalRequired => denied_outcome("approval-required"),
        AppFileDeny::OutsideScope => denied_outcome("outside-scope"),
    }
}

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

#[allow(clippy::result_large_err)]
fn installation_of(query: &HashMap<String, String>) -> Result<String, Response> {
    let value = query
        .get("installationId")
        .ok_or_else(|| error(StatusCode::BAD_REQUEST))?;
    // Reject anything that is not an installation id early; the provider
    // re-checks that the id belongs to the app in the path.
    InstallationId::parse(value).map_err(|_| error(StatusCode::BAD_REQUEST))?;
    Ok(value.clone())
}

#[allow(clippy::result_large_err)]
fn file_key(query: &HashMap<String, String>) -> Result<String, Response> {
    let value = query
        .get("key")
        .ok_or_else(|| error(StatusCode::BAD_REQUEST))?;
    if value.is_empty() || value.len() > MAX_KEY_BYTES {
        return Err(error(StatusCode::BAD_REQUEST));
    }
    Ok(value.clone())
}

#[allow(clippy::result_large_err)]
fn access_of(state: &Arc<GatewayState>) -> Result<Arc<AppAccess>, Response> {
    let apps = state
        .apps
        .clone()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE))?;
    Ok(apps)
}

fn app_error(value: AppAccessError) -> Response {
    match value {
        AppAccessError::Denied => error(StatusCode::NOT_FOUND),
        AppAccessError::Unavailable => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn grants(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let installation = match installation_of(&query) {
        Ok(installation) => installation,
        Err(response) => return response,
    };
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let id_for_call = id.clone();
    let installation_for_call = installation.clone();
    match tokio::task::spawn_blocking(move || {
        apps.app_grants(identity, &id_for_call, &installation_for_call)
    })
    .await
    {
        Ok(Ok(state)) => json_response(
            serde_json::json!({
                "id": id,
                "installationId": installation,
                "requests": state
                    .requests
                    .iter()
                    .map(|request| serde_json::json!({
                        "permission": request.permission,
                        "scope": request.scope,
                        "required": request.required,
                        "reason": request.reason,
                    }))
                    .collect::<Vec<_>>(),
                "grants": state
                    .grants
                    .iter()
                    .map(|grant| serde_json::json!({
                        "id": grant.id,
                        "permission": grant.permission,
                        "scope": grant.scope,
                        "resources": grant.resources,
                        "validity": grant.validity,
                        "expiresAt": grant.expires_at,
                    }))
                    .collect::<Vec<_>>(),
            }),
            StatusCode::OK,
        ),
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn issue(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let installation = match installation_of(&query) {
        Ok(installation) => installation,
        Err(response) => return response,
    };
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/json") {
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
            "permission" | "resources" | "validity" | "expiresIn" | "password"
        )
    }) {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(permission) = value
        .get("permission")
        .and_then(|v| v.as_str())
        .filter(|v| v.len() <= 255)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(resources) = value.get("resources").and_then(|v| v.as_array()) else {
        return error(StatusCode::BAD_REQUEST);
    };
    if resources.is_empty() || resources.len() > MAX_RESOURCES {
        return error(StatusCode::BAD_REQUEST);
    }
    let mut keys = Vec::with_capacity(resources.len());
    for resource in resources {
        let Some(key) = resource
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= MAX_KEY_BYTES)
        else {
            return error(StatusCode::BAD_REQUEST);
        };
        keys.push(key.to_owned());
    }
    let validity = match value.get("validity").and_then(|v| v.as_str()) {
        Some("session") => GrantValidityChoice::Session,
        Some("durable") => GrantValidityChoice::Durable {
            expires_in: match object.get("expiresIn") {
                None => None,
                Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::Number(seconds)) => {
                    let Some(seconds) = seconds.as_u64() else {
                        return error(StatusCode::BAD_REQUEST);
                    };
                    if seconds == 0 || seconds > MAX_CONSENT_SECONDS {
                        return error(StatusCode::BAD_REQUEST);
                    }
                    Some(seconds)
                }
                Some(_) => return error(StatusCode::BAD_REQUEST),
            },
        },
        _ => return error(StatusCode::BAD_REQUEST),
    };
    if validity == GrantValidityChoice::Session && object.contains_key("expiresIn") {
        return error(StatusCode::BAD_REQUEST);
    }

    // A durable write consent is the strongest thing a user can hand an app:
    // it survives restarts and covers the named files until revoked. Proving
    // it is really the account owner costs one password entry.
    let needs_password = permission == "rumahl.files.write"
        && matches!(validity, GrantValidityChoice::Durable { .. });
    let has_password = object.contains_key("password");
    if needs_password != has_password {
        return error(StatusCode::BAD_REQUEST);
    }
    if needs_password {
        let Some(sessions) = state.browser_sessions.clone() else {
            return error(StatusCode::SERVICE_UNAVAILABLE);
        };
        let Some(password) = value["password"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= MAX_PASSWORD_BYTES)
        else {
            return error(StatusCode::BAD_REQUEST);
        };
        let mut password = Zeroizing::new(password.to_owned());
        let user = identity.user_id;
        let verified = tokio::task::spawn_blocking(move || {
            sessions.reauthenticate(user, std::mem::take(&mut *password))
        })
        .await;
        match verified {
            Ok(Ok(())) => {}
            Ok(Err(ShellBackendError::Unauthorized)) => {
                record_audit(
                    &state,
                    rumahl_core::AuditActor::User(identity.user_id),
                    AuditAction::GrantIssued,
                    AuditOutcome::Failure,
                    Some(&format!("{installation}:{permission}")),
                );
                return error(StatusCode::FORBIDDEN);
            }
            _ => return error(StatusCode::SERVICE_UNAVAILABLE),
        }
    }

    let request = GrantIssue {
        permission,
        resources: keys,
        validity,
    };
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let target = format!("{installation}:{}", request.permission);
    let id_for_call = id.clone();
    let installation_for_call = installation.clone();
    match tokio::task::spawn_blocking(move || {
        apps.issue_app_grant(identity, &id_for_call, &installation_for_call, &request)
    })
    .await
    {
        Ok(Ok(grant)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::GrantIssued,
                AuditOutcome::Success,
                Some(&target),
            );
            json_response(
                serde_json::json!({
                    "id": id,
                    "installationId": installation,
                    "grantId": grant.id,
                    "permission": grant.permission,
                    "scope": grant.scope,
                    "resources": grant.resources,
                    "validity": grant.validity,
                    "expiresAt": grant.expires_at,
                }),
                StatusCode::OK,
            )
        }
        Ok(Err(value)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::GrantIssued,
                AuditOutcome::Failure,
                Some(&target),
            );
            app_error(value)
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn revoke(
    State(state): State<Arc<GatewayState>>,
    UrlPath((id, grant)): UrlPath<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let installation = match installation_of(&query) {
        Ok(installation) => installation,
        Err(response) => return response,
    };
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    if grant.is_empty() || grant.len() > 64 {
        return error(StatusCode::BAD_REQUEST);
    }
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let target = format!("{installation}:{grant}");
    match tokio::task::spawn_blocking(move || {
        apps.revoke_app_grant(identity, &id, &installation, &grant)
    })
    .await
    {
        Ok(Ok(())) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::GrantRevoked,
                AuditOutcome::Success,
                Some(&target),
            );
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        Ok(Err(value)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::GrantRevoked,
                AuditOutcome::Failure,
                Some(&target),
            );
            app_error(value)
        }
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn read(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let installation = match installation_of(&query) {
        Ok(installation) => installation,
        Err(response) => return response,
    };
    let key = match file_key(&query) {
        Ok(key) => key,
        Err(response) => return response,
    };
    if query.len() != 2 {
        return error(StatusCode::BAD_REQUEST);
    }
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let access = AppFileAccess {
        action: AppFileAction::Read,
        key: key.clone(),
    };
    let target = format!("{installation}:{key}");
    match tokio::task::spawn_blocking(move || apps.app_file(identity, &id, &installation, &access))
        .await
    {
        Ok(Ok(AppFileOutcome::Content { name, bytes })) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Success,
                Some(&target),
            );
            let mut response = Response::new(axum::body::Body::from(bytes));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/octet-stream"),
            );
            if let Ok(disposition) =
                axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))
            {
                response
                    .headers_mut()
                    .insert(axum::http::header::CONTENT_DISPOSITION, disposition);
            }
            secure_headers(&mut response);
            response
        }
        Ok(Ok(AppFileOutcome::Allowed { .. })) => error(StatusCode::BAD_REQUEST),
        Ok(Ok(AppFileOutcome::Missing)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Failure,
                Some(&target),
            );
            error(StatusCode::NOT_FOUND)
        }
        Ok(Ok(AppFileOutcome::Denied(reason))) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Failure,
                Some(&target),
            );
            file_denied(reason)
        }
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn write(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let identity = match identity_of(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let installation = match installation_of(&query) {
        Ok(installation) => installation,
        Err(response) => return response,
    };
    let key = match file_key(&query) {
        Ok(key) => key,
        Err(response) => return response,
    };
    if query.len() != 2 {
        return error(StatusCode::BAD_REQUEST);
    }
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/octet-stream")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let access = AppFileAccess {
        action: AppFileAction::Write {
            bytes: body.to_vec(),
        },
        key: key.clone(),
    };
    let target = format!("{installation}:{key}");
    match tokio::task::spawn_blocking(move || apps.app_file(identity, &id, &installation, &access))
        .await
    {
        Ok(Ok(AppFileOutcome::Allowed { name, size })) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Success,
                Some(&target),
            );
            json_response(
                serde_json::json!({"outcome": "allowed", "name": name, "size": size}),
                StatusCode::OK,
            )
        }
        Ok(Ok(AppFileOutcome::Content { .. })) => error(StatusCode::BAD_REQUEST),
        Ok(Ok(AppFileOutcome::Missing)) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Failure,
                Some(&target),
            );
            error(StatusCode::NOT_FOUND)
        }
        Ok(Ok(AppFileOutcome::Denied(reason))) => {
            record_audit(
                &state,
                rumahl_core::AuditActor::User(identity.user_id),
                AuditAction::AppFileAccess,
                AuditOutcome::Failure,
                Some(&target),
            );
            file_denied(reason)
        }
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
