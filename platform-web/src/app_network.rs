//! Generic brokered HTTP(S) egress for installed apps.
//!
//! An app's iframe is sandboxed with an opaque origin and a `connect-src`
//! policy limited to its own asset host, so it cannot reach a LAN service
//! directly. This route is the controlled alternative: the app supplies a
//! request, the OS checks the signed manifest and the user's explicit origin
//! consent, attaches a configured secret server-side, and performs the call.
//!
//! Hard limits here are independent of the app:
//! * HTTP(S) only, no credentials in the URL, no fragments;
//! * no redirects (a 30x is returned to the app as-is);
//! * link-local/unspecified/multicast destinations are refused, even after
//!   DNS resolution, so the broker cannot be aimed at metadata services;
//! * private/loopback destinations need a separate consent on the origin
//!   grant; a public grant cannot reach them through DNS;
//! * a small method/header/body/response budget;
//! * one resolved address is pinned for the request, so DNS cannot change
//!   the destination between the check and the connection.

use crate::apps::{AppNetworkRequest, GrantValidityChoice};
use crate::{
    AppAccessError, AppNetworkError, GatewayState, ShellBackendError, ShellIdentity,
    server::{authenticate, credential, error, record_audit, secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path as UrlPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;
use rumahl_core::network::{allowed, private};
use rumahl_core::{AuditAction, AuditActor, AuditOutcome, InstallationId};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const MAX_CONSENT_BYTES: usize = 4 * 1024;
const MAX_REQUEST_BYTES: usize = 512 * 1024;
const MAX_HEADERS: usize = 16;
const MAX_HEADER_VALUE: usize = 1024;
const MAX_ORIGIN_BYTES: usize = 512;
const MAX_SECRET_KEY_BYTES: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CONSENT_SECONDS: u64 = 31_536_000;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route(
            "/api/v1/shell/apps/{id}/network/consent",
            get(list_origins)
                .put(grant_origin)
                .delete(revoke_origin)
                .layer(DefaultBodyLimit::max(MAX_CONSENT_BYTES)),
        )
        .route(
            "/api/v1/shell/apps/{id}/network/request",
            post(forward).layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES)),
        )
}

fn json_response(value: serde_json::Value, status: StatusCode) -> Response {
    let mut response = (status, axum::Json(value)).into_response();
    secure_headers(&mut response);
    response
}

fn denied(reason: &str) -> Response {
    json_response(
        serde_json::json!({"outcome": "denied", "reason": reason}),
        StatusCode::FORBIDDEN,
    )
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
    InstallationId::parse(value).map_err(|_| error(StatusCode::BAD_REQUEST))?;
    Ok(value.clone())
}

#[allow(clippy::result_large_err)]
fn access_of(state: &Arc<GatewayState>) -> Result<Arc<crate::AppAccess>, Response> {
    state
        .apps
        .clone()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE))
}

fn app_error(value: AppAccessError) -> Response {
    match value {
        AppAccessError::Denied => error(StatusCode::NOT_FOUND),
        AppAccessError::Unavailable => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn origin_info(origin: &crate::AppNetworkOrigin) -> serde_json::Value {
    serde_json::json!({
        "origin": origin.origin,
        "validity": origin.validity,
        "expiresAt": origin.expires_at,
        "allowPrivate": origin.allow_private,
    })
}

async fn list_origins(
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
    match tokio::task::spawn_blocking(move || apps.network_origins(identity, &id, &installation))
        .await
    {
        Ok(Ok(origins)) => json_response(
            serde_json::json!({
                "origins": origins.iter().map(origin_info).collect::<Vec<_>>(),
            }),
            StatusCode::OK,
        ),
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn grant_origin(
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
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "origin" | "validity" | "expiresIn" | "allowPrivate"
        )
    }) {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(origin) = value
        .get("origin")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty() && value.len() <= MAX_ORIGIN_BYTES)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let allow_private = match value.get("allowPrivate") {
        None | Some(serde_json::Value::Bool(false)) => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
    let validity = match value.get("validity").and_then(|value| value.as_str()) {
        Some("session") => {
            if object.contains_key("expiresIn") {
                return error(StatusCode::BAD_REQUEST);
            }
            GrantValidityChoice::Session
        }
        Some("durable") => {
            let expires_in = match object.get("expiresIn") {
                None | Some(serde_json::Value::Null) => None,
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
            };
            if !state
                .policies
                .clone()
                .is_none_or(|policies| policies.durable_grants_allowed(identity.user_id))
            {
                record_audit(
                    &state,
                    AuditActor::User(identity.user_id),
                    AuditAction::GrantIssued,
                    AuditOutcome::Denied,
                    Some(&format!("{installation}:network")),
                );
                return json_response(
                    serde_json::json!({"outcome": "denied", "reason": "durable-grants-disabled"}),
                    StatusCode::FORBIDDEN,
                );
            }
            GrantValidityChoice::Durable { expires_in }
        }
        _ => return error(StatusCode::BAD_REQUEST),
    };
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let installation_for_call = installation.clone();
    match tokio::task::spawn_blocking(move || {
        apps.grant_network_origin(
            identity,
            &id,
            &installation_for_call,
            &origin,
            validity,
            allow_private,
        )
    })
    .await
    {
        Ok(Ok(origin)) => {
            record_audit(
                &state,
                AuditActor::User(identity.user_id),
                AuditAction::GrantIssued,
                AuditOutcome::Success,
                Some(&format!(
                    "{installation}:network:{}:{}",
                    origin.origin,
                    if origin.allow_private {
                        "private"
                    } else {
                        "public"
                    }
                )),
            );
            json_response(origin_info(&origin), StatusCode::OK)
        }
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn revoke_origin(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
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
    let Some(installation) = query.get("installationId").cloned() else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(origin) = query.get("origin").cloned() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if query.len() != 2
        || InstallationId::parse(&installation).is_err()
        || origin.is_empty()
        || origin.len() > MAX_ORIGIN_BYTES
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let target = format!("{installation}:network:{origin}");
    match tokio::task::spawn_blocking(move || {
        apps.revoke_network_origin(identity, &id, &installation, &origin)
    })
    .await
    {
        Ok(Ok(())) => {
            record_audit(
                &state,
                AuditActor::User(identity.user_id),
                AuditAction::GrantRevoked,
                AuditOutcome::Success,
                Some(&target),
            );
            json_response(serde_json::json!({"revoked": true}), StatusCode::OK)
        }
        Ok(Err(value)) => app_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn forward(
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
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(object) = value.as_object() else {
        return error(StatusCode::BAD_REQUEST);
    };
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "url" | "method" | "headers" | "body" | "secret" | "maxBytes"
        )
    }) {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(url) = value
        .get("url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .map(str::to_owned)
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let method = match value.get("method") {
        None => "GET".to_owned(),
        Some(serde_json::Value::String(method)) if method.len() <= 16 => {
            method.to_ascii_uppercase()
        }
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
    let mut request_headers = Vec::new();
    match object.get("headers") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Object(map)) => {
            if map.len() > MAX_HEADERS {
                return error(StatusCode::BAD_REQUEST);
            }
            for (name, value) in map {
                let Some(value) = value.as_str() else {
                    return error(StatusCode::BAD_REQUEST);
                };
                if name.len() > 128 || value.len() > MAX_HEADER_VALUE {
                    return error(StatusCode::BAD_REQUEST);
                }
                request_headers.push((name.clone(), value.to_owned()));
            }
        }
        Some(_) => return error(StatusCode::BAD_REQUEST),
    }
    let request_body = match object.get("body") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(text)) if text.len() <= MAX_REQUEST_BYTES => {
            Some(text.as_bytes().to_vec())
        }
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
    let secret = match object.get("secret") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(key))
            if !key.is_empty() && key.len() <= MAX_SECRET_KEY_BYTES =>
        {
            Some(key.clone())
        }
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
    let max_bytes = match object.get("maxBytes") {
        None | Some(serde_json::Value::Null) => 0,
        Some(serde_json::Value::Number(value)) => value.as_u64().unwrap_or(0) as usize,
        Some(_) => return error(StatusCode::BAD_REQUEST),
    };
    let apps = match access_of(&state) {
        Ok(apps) => apps,
        Err(response) => return response,
    };
    let request = AppNetworkRequest {
        url,
        method,
        headers: request_headers,
        body: request_body,
        secret,
        max_bytes,
    };
    let id_for_call = id.clone();
    let installation_for_call = installation.clone();
    let prepared = match tokio::task::spawn_blocking(move || {
        apps.prepare_network_request(identity, &id_for_call, &installation_for_call, &request)
    })
    .await
    {
        Ok(Ok(prepared)) => prepared,
        Ok(Err(AppNetworkError::ConsentRequired { origin })) => {
            return json_response(
                serde_json::json!({
                    "outcome": "denied",
                    "reason": "approval-required",
                    "origin": origin,
                }),
                StatusCode::FORBIDDEN,
            );
        }
        Ok(Err(AppNetworkError::PrivateConsentRequired { origin })) => {
            return json_response(
                serde_json::json!({
                    "outcome": "denied",
                    "reason": "private-approval-required",
                    "origin": origin,
                }),
                StatusCode::FORBIDDEN,
            );
        }
        Ok(Err(AppNetworkError::Denied)) => {
            record_audit(
                &state,
                AuditActor::App(id.clone()),
                AuditAction::CapabilityDenied,
                AuditOutcome::Denied,
                Some(&format!("network:{installation}")),
            );
            return denied("not-permitted");
        }
        Ok(Err(AppNetworkError::Blocked)) => {
            record_audit(
                &state,
                AuditActor::App(id.clone()),
                AuditAction::CapabilityDenied,
                AuditOutcome::Denied,
                Some(&format!("network:{installation}")),
            );
            return denied("blocked-target");
        }
        Ok(Err(AppNetworkError::Unavailable)) => {
            return error(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    match execute(prepared).await {
        Ok((status, content_type, body, truncated)) => {
            let mut response = json_response(
                serde_json::json!({
                    "networkVersion": 1,
                    "status": status,
                    "headers": content_type.map(|value| serde_json::json!({"content-type": value})),
                    "body": String::from_utf8_lossy(&body),
                    "truncated": truncated,
                }),
                StatusCode::OK,
            );
            response.headers_mut().insert(
                "cache-control",
                axum::http::HeaderValue::from_static("no-store"),
            );
            response
        }
        Err(ExecuteError::PrivateConsent { origin }) => json_response(
            serde_json::json!({
                "outcome": "denied",
                "reason": "private-approval-required",
                "origin": origin,
            }),
            StatusCode::FORBIDDEN,
        ),
        Err(ExecuteError::Blocked) => denied("blocked-target"),
        Err(ExecuteError::Unavailable) => denied("upstream-unavailable"),
    }
}

#[derive(Debug)]
enum ExecuteError {
    Blocked,
    PrivateConsent { origin: String },
    Unavailable,
}

/// Performs the prepared request with a pinned destination and a bounded body.
async fn execute(
    request: crate::PreparedNetworkRequest,
) -> Result<(u16, Option<String>, Vec<u8>, bool), ExecuteError> {
    let url = Url::parse(&request.url).map_err(|_| ExecuteError::Unavailable)?;
    let host = url.host_str().ok_or(ExecuteError::Unavailable)?;
    let port = url
        .port_or_known_default()
        .ok_or(ExecuteError::Unavailable)?;
    let addresses: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| ExecuteError::Unavailable)?
            .collect()
    };
    if !request.allow_private && addresses.iter().any(|address| private(address.ip())) {
        return Err(ExecuteError::PrivateConsent {
            origin: url.origin().ascii_serialization(),
        });
    }
    let address = addresses
        .into_iter()
        .find(|address| allowed(address.ip(), request.allow_private))
        .ok_or(ExecuteError::Blocked)?;
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    // Host names are pinned to the address that was checked above. A literal
    // IP is already the destination; resolving it again is unnecessary and
    // would reject bracketed IPv6 forms.
    if host.parse::<IpAddr>().is_err() {
        builder = builder.resolve(host, address);
    }
    let client = builder.build().map_err(|_| ExecuteError::Unavailable)?;
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|_| ExecuteError::Unavailable)?;
    let mut builder = client.request(method, &request.url);
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }
    let response = builder
        .send()
        .await
        .map_err(|_| ExecuteError::Unavailable)?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return Err(ExecuteError::Unavailable);
        };
        if body.len() + chunk.len() > request.max_bytes {
            let remaining = request.max_bytes.saturating_sub(body.len());
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok((status, content_type, body, truncated))
}
