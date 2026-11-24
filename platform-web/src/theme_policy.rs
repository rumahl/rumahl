//! Device policy for executable themes.
//!
//! The shell fetches this before offering full shell access. It is a guardrail
//! for the UI and the theme runtime; the per-theme consent remains the explicit
//! user decision.
use crate::{GatewayState, preferences::user, server::secure_headers};
use axum::{
    Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
};
use std::sync::Arc;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new().route("/api/v1/shell/theme-policy", get(load))
}

async fn load(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let user = match user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let allowed = state
        .policies
        .as_ref()
        .is_none_or(|policies| policies.full_access_themes_allowed());
    let mut response = axum::Json(serde_json::json!({
        "settingsVersion": 1,
        "ownerId": user.to_string(),
        "allowFullAccessThemes": allowed,
    }))
    .into_response();
    secure_headers(&mut response);
    response
}
