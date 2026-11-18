//! Extended platform configuration view.
//!
//! Deployments configure the platform through the environment written to
//! `/etc/rumahl/platform.env`; the service inherits it at startup. This view
//! reports the effective configuration to advanced users. It is built once
//! from the process environment at startup — never per request — and it only
//! ever contains entries from a fixed allow-list of non-sensitive keys. Every
//! value additionally passes a name-based secret filter, so a key that sounds
//! like a credential can never be displayed even if the allow-list grows.

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
use std::{collections::HashMap, sync::Arc};

/// Bounded per-entry values; anything longer is truncated, never dropped, so
/// the count the user sees always matches what the deployment set.
const MAX_VALUE_BYTES: usize = 512;
/// Upper bound on reported entries.
const MAX_ENTRIES: usize = 64;

/// The deployment keys the view reports. Anything else — secrets in
/// particular — is invisible, including its existence.
const REPORTED_KEYS: &[&str] = &[
    "RUMAHL_PUBLIC_ORIGIN",
    "RUMAHL_APP_HOST_SUFFIX",
    "RUMAHL_STATE_DIR",
    "RUMAHL_LOCALE",
    "RUMAHL_APPS_ROOT",
    "RUMAHL_SYSTEM_ROOT",
    "RUMAHL_FILE_ENCRYPTION",
    "RUMAHL_GATEWAY_SOCKET_ACCESS",
    "RUMAHL_PASSWORD_BLOCKLIST",
    "RUMAHL_CLIENT_BUILD",
    "RUMAHL_SSR_SOCKET",
];

/// One reported configuration entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformConfigEntry {
    pub name: String,
    pub value: String,
}

/// The platform's effective, display-safe configuration, captured at startup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlatformConfig {
    entries: Vec<PlatformConfigEntry>,
}

impl PlatformConfig {
    /// Reads the reported keys from the process environment, once. Unknown and
    /// secret-sounding variables are ignored without a trace.
    pub fn from_environment() -> Self {
        let mut entries = Vec::new();
        for name in REPORTED_KEYS {
            if !is_reportable(name) {
                continue;
            }
            let Some(value) = std::env::var(name).ok() else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            entries.push(PlatformConfigEntry {
                name: (*name).to_owned(),
                value: truncate(&value),
            });
        }
        entries.truncate(MAX_ENTRIES);
        Self { entries }
    }

    /// A fixed configuration, for tests and embedding. Behaves exactly like
    /// the captured one: only allow-listed, non-secret keys survive.
    pub fn from_entries(entries: Vec<PlatformConfigEntry>) -> Self {
        let entries = entries
            .into_iter()
            .filter(|entry| {
                REPORTED_KEYS.contains(&entry.name.as_str()) && is_reportable(&entry.name)
            })
            .map(|entry| PlatformConfigEntry {
                name: entry.name,
                value: truncate(&entry.value),
            })
            .collect();
        Self { entries }
    }

    pub fn entries(&self) -> &[PlatformConfigEntry] {
        &self.entries
    }
}

/// Names that could carry a credential are never shown, allow-list or not.
/// Names that could carry a credential are never shown, allow-list or not.
/// Shared by the platform configuration and the recorded app image
/// environment, so one rule decides what an extended configuration view may
/// ever display.
pub fn is_display_safe_name(name: &str) -> bool {
    is_reportable(name)
}

fn is_reportable(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    ["SECRET", "PASSWORD", "TOKEN", "KEY"]
        .iter()
        .all(|needle| !upper.contains(needle))
}

fn truncate(value: &str) -> String {
    if value.len() <= MAX_VALUE_BYTES {
        return value.to_owned();
    }
    // Cut on a character boundary, never mid-UTF-8.
    let mut end = MAX_VALUE_BYTES;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new().route("/api/v1/shell/config", get(load))
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
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(device) = query
        .get("device")
        .and_then(|value| rumahl_core::BrowserProfileId::parse(value))
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    // The extended configuration is an advanced surface: same gate as the
    // host file browser.
    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mode = match tokio::task::spawn_blocking(move || mode_repository.load(user, device)).await {
        Ok(Ok(settings)) => settings.effective_mode(),
        Ok(Err(_)) => return error(StatusCode::SERVICE_UNAVAILABLE),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
    };
    if !mode.policy().can_view_platform_config() {
        return error(StatusCode::FORBIDDEN);
    }
    let Some(config) = state.platform_config.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut response = axum::Json(serde_json::json!({
        "configVersion": 1,
        "entries": config
            .entries()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_secret_sounding_names_even_when_listed() {
        // A key that could carry a credential never appears, even if someone
        // adds it to the allow-list.
        assert!(!is_reportable("RUMAHL_SECRET_KEY_ID"));
        assert!(!is_reportable("RUMAHL_OAUTH_TOKEN"));
        let config = PlatformConfig::from_entries(vec![
            PlatformConfigEntry {
                name: "RUMAHL_PUBLIC_ORIGIN".into(),
                value: "https://rumahl.test".into(),
            },
            PlatformConfigEntry {
                name: "RUMAHL_SECRET_KEY_ID".into(),
                value: "sealed-handle".into(),
            },
        ]);

        assert_eq!(
            config.entries(),
            &[PlatformConfigEntry {
                name: "RUMAHL_PUBLIC_ORIGIN".into(),
                value: "https://rumahl.test".into(),
            }]
        );
    }

    #[test]
    fn truncates_long_values_on_character_boundaries() {
        let config = PlatformConfig::from_entries(vec![PlatformConfigEntry {
            name: "RUMAHL_LOCALE".into(),
            value: "ä".repeat(600),
        }]);

        assert!(config.entries()[0].value.len() <= MAX_VALUE_BYTES);
        assert!(config.entries()[0].value.chars().all(|c| c == 'ä'));
    }

    #[test]
    fn keeps_unreportable_keys_out_entirely() {
        let config = PlatformConfig::from_entries(vec![PlatformConfigEntry {
            name: "PATH".into(),
            value: "/usr/bin".into(),
        }]);

        assert!(config.entries().is_empty());
    }
}
