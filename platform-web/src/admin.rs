//! Authenticated account and session administration for the shell settings UI.
use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path as UrlPath, Query, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
};
use rumahl_core::{AuditOutcome, SessionId, UserId, UserRole};
use rumahl_ui_contracts::ShellUserRole;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::server::{GatewayState, authenticate, credential, error, secure_headers, valid_origin};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub status: String,
    pub role: ShellUserRole,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminSession {
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub authenticated_at: u64,
    pub last_seen_at: u64,
    pub expires_at: u64,
    pub revoked_at: Option<u64>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuditEntry {
    pub sequence: u64,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub outcome: String,
    pub target: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAdminUser {
    pub username: String,
    pub display_name: String,
    pub password: String,
    #[serde(default)]
    pub role: Option<ShellUserRole>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAdminUser {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub role: Option<ShellUserRole>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetAdminPassword {
    pub password: String,
}

/// One policy assignment as the administration UI sees it. `value` is `null`
/// after a clear request; the catalog default then applies.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPolicyAssignment {
    pub subject: String,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminGroup {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetAdminPolicy {
    /// `device`, `group:<uuid>` or `user:<uuid>`.
    pub subject: String,
    pub key: String,
    /// `null` clears the assignment.
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAdminGroup {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAdminGroupMembers {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminError {
    Forbidden,
    NotFound,
    Conflict,
    Invalid,
    Unavailable,
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Forbidden => "administrator role required",
            Self::NotFound => "account or session not found",
            Self::Conflict => "administration request conflicts with current state",
            Self::Invalid => "invalid administration request",
            Self::Unavailable => "administration unavailable",
        })
    }
}

impl std::error::Error for AdminError {}

/// Persisted account administration. Implementations must authorize the actor
/// on every mutating call; HTTP handlers also require an administrator role.
pub trait Administration: Send + Sync + 'static {
    fn role(&self, user_id: UserId) -> Result<UserRole, AdminError>;
    fn list_users(&self, actor: UserId) -> Result<Vec<AdminUser>, AdminError>;
    fn create_user(
        &self,
        actor: UserId,
        username: &str,
        display_name: &str,
        password: String,
        role: ShellUserRole,
    ) -> Result<AdminUser, AdminError>;
    fn update_user(
        &self,
        actor: UserId,
        user_id: UserId,
        status: Option<String>,
        role: Option<ShellUserRole>,
        username: Option<String>,
        display_name: Option<String>,
    ) -> Result<AdminUser, AdminError>;
    fn delete_user(&self, actor: UserId, user_id: UserId) -> Result<(), AdminError>;
    fn reset_password(
        &self,
        actor: UserId,
        user_id: UserId,
        password: String,
    ) -> Result<(), AdminError>;
    fn list_sessions(&self, actor: UserId) -> Result<Vec<AdminSession>, AdminError>;
    fn revoke_session(&self, actor: UserId, session_id: SessionId) -> Result<(), AdminError>;
    fn audit(
        &self,
        actor: UserId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AdminAuditEntry>, AdminError>;
    /// Every stored policy assignment, for the administration view.
    fn list_policy_assignments(
        &self,
        actor: UserId,
    ) -> Result<Vec<AdminPolicyAssignment>, AdminError>;
    /// Stores or clears one assignment. The key must be in the catalog and the
    /// subject must exist; anything else is refused.
    fn set_policy_assignment(
        &self,
        actor: UserId,
        subject: rumahl_core::PolicySubject,
        key: rumahl_core::PolicyKey,
        value: Option<rumahl_core::PolicyValue>,
    ) -> Result<(), AdminError>;
    fn list_groups(&self, actor: UserId) -> Result<Vec<AdminGroup>, AdminError>;
    fn create_group(&self, actor: UserId, name: &str) -> Result<AdminGroup, AdminError>;
    fn delete_group(&self, actor: UserId, id: rumahl_core::GroupId) -> Result<(), AdminError>;
    fn update_group_members(
        &self,
        actor: UserId,
        id: rumahl_core::GroupId,
        add: Vec<UserId>,
        remove: Vec<UserId>,
    ) -> Result<AdminGroup, AdminError>;
}

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/admin/users", get(users).post(create))
        .route(
            "/api/v1/admin/users/{id}",
            patch(update).delete(remove_user),
        )
        .route("/api/v1/admin/users/{id}/password", post(password))
        .route("/api/v1/admin/sessions", get(sessions))
        .route("/api/v1/admin/sessions/{id}", delete(revoke))
        .route("/api/v1/admin/audit", get(audit))
        .route("/api/v1/admin/policies", get(policies).put(set_policy))
        .route("/api/v1/admin/groups", get(groups).post(create_group))
        .route("/api/v1/admin/groups/{id}", delete(delete_group))
        .route("/api/v1/admin/groups/{id}/members", put(update_members))
        .layer(DefaultBodyLimit::max(8192))
}

async fn actor(state: &Arc<GatewayState>, headers: &HeaderMap) -> Result<UserId, Response> {
    let token = credential(headers).map_err(|_| error(StatusCode::UNAUTHORIZED))?;
    let identity = authenticate(state, &token)
        .await
        .map_err(|_| error(StatusCode::UNAUTHORIZED))?;
    let admin = state
        .admin
        .clone()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE))?;
    let role = tokio::task::spawn_blocking(move || admin.role(identity.user_id))
        .await
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE))?
        .map_err(admin_error)?;
    if !role.is_administrator() {
        return Err(error(StatusCode::FORBIDDEN));
    }
    Ok(identity.user_id)
}

fn admin_error(error_value: AdminError) -> Response {
    let status = match error_value {
        AdminError::Forbidden => StatusCode::FORBIDDEN,
        AdminError::NotFound => StatusCode::NOT_FOUND,
        AdminError::Conflict => StatusCode::CONFLICT,
        AdminError::Invalid => StatusCode::BAD_REQUEST,
        AdminError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    error(status)
}

#[allow(clippy::result_large_err)]
fn admin(state: &Arc<GatewayState>) -> Result<Arc<dyn Administration>, Response> {
    state
        .admin
        .clone()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE))
}

#[allow(clippy::result_large_err)]
fn body<T: DeserializeOwned>(headers: &HeaderMap, bytes: &Bytes) -> Result<T, Response> {
    if headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or("").trim())
        != Some("application/json")
    {
        return Err(error(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }
    serde_json::from_slice(bytes).map_err(|_| error(StatusCode::BAD_REQUEST))
}

#[allow(clippy::result_large_err)]
fn origin(state: &GatewayState, headers: &HeaderMap) -> Result<(), Response> {
    valid_origin(headers, &state.config.public_origin)
        .then_some(())
        .ok_or_else(|| error(StatusCode::FORBIDDEN))
}

fn json<T: Serialize>(value: T) -> Response {
    let mut response = Json(value).into_response();
    secure_headers(&mut response);
    response
}

async fn users(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.list_users(actor)).await {
        Ok(Ok(users)) => json(serde_json::json!({ "users": users })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn create(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let input: CreateAdminUser = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || {
        admin.create_user(
            actor,
            &input.username,
            &input.display_name,
            input.password,
            input.role.unwrap_or(ShellUserRole::User),
        )
    })
    .await
    {
        Ok(Ok(user)) => json(user),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn update(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(user_id) = UserId::parse(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let input: UpdateAdminUser = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || {
        admin.update_user(
            actor,
            user_id,
            input.status,
            input.role,
            input.username,
            input.display_name,
        )
    })
    .await
    {
        Ok(Ok(user)) => json(user),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn remove_user(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(user_id) = UserId::parse(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.delete_user(actor, user_id)).await {
        Ok(Ok(())) => json(serde_json::json!({ "ok": true })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn password(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(user_id) = UserId::parse(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let input: ResetAdminPassword = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.reset_password(actor, user_id, input.password))
        .await
    {
        Ok(Ok(())) => json(serde_json::json!({ "ok": true })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn sessions(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.list_sessions(actor)).await {
        Ok(Ok(sessions)) => json(serde_json::json!({ "sessions": sessions })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn revoke(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(session_id) = SessionId::parse(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.revoke_session(actor, session_id)).await {
        Ok(Ok(())) => json(serde_json::json!({ "ok": true })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuditParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

async fn audit(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<AuditParams>,
    headers: HeaderMap,
) -> Response {
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).min(10_000);
    match tokio::task::spawn_blocking(move || admin.audit(actor, limit, offset)).await {
        Ok(Ok(events)) => json(serde_json::json!({ "events": events })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn policy_json(assignments: Vec<AdminPolicyAssignment>) -> serde_json::Value {
    let policies: Vec<serde_json::Value> = rumahl_core::policy_catalog()
        .iter()
        .map(|definition| {
            serde_json::json!({
                "key": definition.key,
                "kind": definition.kind.as_str(),
                "default": match definition.default {
                    rumahl_core::PolicyValue::Boolean(value) => serde_json::json!(value),
                    rumahl_core::PolicyValue::Number(value) => serde_json::json!(value),
                },
                "minimum": if definition.minimum == 0 { serde_json::Value::Null } else { serde_json::json!(definition.minimum) },
                "maximum": if definition.maximum == 0 { serde_json::Value::Null } else { serde_json::json!(definition.maximum) },
                "description": definition.description,
            })
        })
        .collect();
    serde_json::json!({ "policiesVersion": 1, "policies": policies, "assignments": assignments })
}

async fn policies(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.list_policy_assignments(actor)).await {
        Ok(Ok(assignments)) => json(policy_json(assignments)),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn decode_assignment(
    input: &SetAdminPolicy,
) -> Result<
    (
        rumahl_core::PolicySubject,
        rumahl_core::PolicyKey,
        Option<rumahl_core::PolicyValue>,
    ),
    AdminError,
> {
    let subject = rumahl_core::PolicySubject::decode(&input.subject).ok_or(AdminError::Invalid)?;
    let key = rumahl_core::PolicyKey::parse(&input.key).map_err(|_| AdminError::Invalid)?;
    let value = match input.value.as_ref() {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Bool(value)) => Some(rumahl_core::PolicyValue::Boolean(*value)),
        Some(serde_json::Value::Number(number)) => Some(rumahl_core::PolicyValue::Number(
            number.as_u64().ok_or(AdminError::Invalid)?,
        )),
        Some(_) => return Err(AdminError::Invalid),
    };
    Ok((subject, key, value))
}

async fn set_policy(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let input: SetAdminPolicy = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let (subject, key, value) = match decode_assignment(&input) {
        Ok(decoded) => decoded,
        Err(error_value) => return admin_error(error_value),
    };
    let target = format!("policy:{}:{}", subject.encode(), key);
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    let outcome = match tokio::task::spawn_blocking(move || {
        admin.set_policy_assignment(actor, subject, key, value)
    })
    .await
    {
        Ok(result) => match result {
            Ok(()) => AuditOutcome::Success,
            Err(error_value) => return audit_policy_change(&state, actor, &target, error_value),
        },
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    let _ = outcome;
    crate::server::record_audit(
        &state,
        rumahl_core::AuditActor::User(actor),
        rumahl_core::AuditAction::Maintenance,
        AuditOutcome::Success,
        Some(&target),
    );
    error(StatusCode::NO_CONTENT)
}

#[allow(clippy::result_large_err)]
fn audit_policy_change(
    state: &Arc<GatewayState>,
    actor: UserId,
    target: &str,
    error_value: AdminError,
) -> Response {
    crate::server::record_audit(
        state,
        rumahl_core::AuditActor::User(actor),
        rumahl_core::AuditAction::Maintenance,
        AuditOutcome::Failure,
        Some(target),
    );
    admin_error(error_value)
}

async fn groups(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.list_groups(actor)).await {
        Ok(Ok(groups)) => json(serde_json::json!({ "groups": groups })),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn create_group(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let input: CreateAdminGroup = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.create_group(actor, &input.name)).await {
        Ok(Ok(group)) => {
            crate::server::record_audit(
                &state,
                rumahl_core::AuditActor::User(actor),
                rumahl_core::AuditAction::Maintenance,
                AuditOutcome::Success,
                Some(&format!("group:{}", group.name)),
            );
            json(group)
        }
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[allow(clippy::result_large_err)]
fn group_id(raw: &str) -> Result<rumahl_core::GroupId, Response> {
    rumahl_core::GroupId::parse(raw).map_err(|_| error(StatusCode::BAD_REQUEST))
}

async fn delete_group(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(id) = group_id(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.delete_group(actor, id)).await {
        Ok(Ok(())) => {
            crate::server::record_audit(
                &state,
                rumahl_core::AuditActor::User(actor),
                rumahl_core::AuditAction::Maintenance,
                AuditOutcome::Success,
                Some("group"),
            );
            error(StatusCode::NO_CONTENT)
        }
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn update_members(
    State(state): State<Arc<GatewayState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(response) = origin(&state, &headers) {
        return response;
    }
    let actor = match actor(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let Ok(id) = group_id(&id) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let input: UpdateAdminGroupMembers = match body(&headers, &bytes) {
        Ok(input) => input,
        Err(response) => return response,
    };
    let add = match input
        .add
        .iter()
        .map(|value| UserId::parse(value))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(add) => add,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let remove = match input
        .remove
        .iter()
        .map(|value| UserId::parse(value))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(remove) => remove,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let admin = match admin(&state) {
        Ok(admin) => admin,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || admin.update_group_members(actor, id, add, remove))
        .await
    {
        Ok(Ok(group)) => json(group),
        Ok(Err(error_value)) => admin_error(error_value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
