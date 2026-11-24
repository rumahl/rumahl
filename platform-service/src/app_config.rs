//! Persisted app configuration: manifest setting values and network consent.
//!
//! This module is the authorization side of the two generic app resources an
//! integration app needs: its own settings (including write-only secrets) and
//! brokered HTTP(S) egress to origins the user approved. The transport that
//! actually opens a socket lives in `rumahl-platform-web`; nothing here accepts
//! a target from the app without checking the live installation, the signed
//! manifest and the user's consent.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

use rumahl_core::network::{blocked, private};
use rumahl_core::{
    APP_NETWORK_PERMISSION, AppSettingKind, InstallationId, SessionId, UnixTimestamp,
};
use rumahl_persistence_sqlite::AppSettingWrite;
use rumahl_platform_web::{
    AppAccessError, AppNetworkError, AppNetworkOrigin, AppNetworkRequest, GrantValidityChoice,
    PreparedNetworkRequest, ShellIdentity,
};
use url::Url;

use crate::apps::PersistentApps;

/// Largest plain setting value. Dashboard layouts fit comfortably; this is
/// configuration, not storage.
const MAX_SETTING_BYTES: usize = 64 * 1024;
/// Largest secret value (an access token, a client key).
const MAX_SECRET_BYTES: usize = 16 * 1024;
/// Largest request body an app may send through the network broker.
const MAX_REQUEST_BODY_BYTES: usize = 256 * 1024;
/// Default and maximum response body. A dashboard state document is a few
/// hundred KiB; this keeps a hostile endpoint from exhausting memory.
const DEFAULT_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
/// One year; a network consent is a decision, not a permanent policy.
const MAX_NETWORK_CONSENT_SECONDS: u64 = 31_536_000;

/// Request headers an app may set. Authentication is deliberately absent:
/// only a declared secret setting can become `Authorization`, server-side.
const ALLOWED_HEADERS: [&str; 5] = [
    "accept",
    "content-type",
    "cache-control",
    "if-none-match",
    "if-modified-since",
];

fn now_seconds() -> u64 {
    UnixTimestamp::now()
        .map(|time| time.as_seconds())
        .unwrap_or(0)
}

/// Canonical origin of a request URL. The URL itself may carry a path and
/// query; the consent is always for its origin only.
fn request_origin(url: &Url) -> Option<String> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

/// Canonical origin supplied by a consent dialog. Paths, queries, fragments
/// and credentials are not origins and are rejected rather than normalized to
/// a broader grant.
fn consent_origin(value: &str) -> Option<String> {
    if value.is_empty() || value.len() > 512 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.path() != "/" || url.query().is_some() {
        return None;
    }
    request_origin(&url)
}

fn has_network_permission(app: &rumahl_core::InstalledAppSnapshot) -> bool {
    app.manifest().permission_requests().iter().any(|request| {
        request.permission().as_str() == APP_NETWORK_PERMISSION
            && matches!(
                request.requested_scope(),
                rumahl_core::PermissionScope::UserSelected | rumahl_core::PermissionScope::Explicit
            )
    })
}

fn store_error<T>(value: Result<T, impl std::fmt::Debug>) -> Result<T, AppAccessError> {
    value.map_err(|_| AppAccessError::Unavailable)
}

impl PersistentApps {
    fn settings_store(
        &self,
    ) -> Result<&std::sync::Arc<rumahl_persistence_sqlite::SqliteAppSettings>, AppAccessError> {
        self.settings.as_ref().ok_or(AppAccessError::Unavailable)
    }

    fn network_store(
        &self,
    ) -> Result<&std::sync::Arc<rumahl_persistence_sqlite::SqliteNetworkGrants>, AppAccessError>
    {
        self.network.as_ref().ok_or(AppAccessError::Unavailable)
    }

    pub(crate) fn app_settings_values_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<HashMap<String, serde_json::Value>, AppAccessError> {
        let app = self.app(identity, installation)?;
        let store = self.settings_store()?;
        let declared: HashSet<String> = app
            .manifest()
            .settings()
            .iter()
            .map(|setting| setting.key().as_str().to_owned())
            .collect();
        let mut values = HashMap::new();
        for stored in store_error(store.load(&installation.to_string()))? {
            if stored.secret || !declared.contains(&stored.key) {
                continue;
            }
            let Some(text) = stored.value_text else {
                continue;
            };
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                values.insert(stored.key, value);
            }
        }
        Ok(values)
    }

    pub(crate) fn app_setting_secrets_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<Vec<String>, AppAccessError> {
        let app = self.app(identity, installation)?;
        let store = self.settings_store()?;
        let declared: HashSet<String> = app
            .manifest()
            .settings()
            .iter()
            .filter(|setting| matches!(setting.kind(), AppSettingKind::Secret))
            .map(|setting| setting.key().as_str().to_owned())
            .collect();
        let mut keys: Vec<String> = store_error(store.load(&installation.to_string()))?
            .into_iter()
            .filter(|stored| stored.secret && declared.contains(&stored.key))
            .map(|stored| stored.key)
            .collect();
        keys.sort();
        Ok(keys)
    }

    pub(crate) fn update_app_settings_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        values: HashMap<String, serde_json::Value>,
    ) -> Result<(), AppAccessError> {
        let app = self.app(identity, installation)?;
        let store = self.settings_store()?;
        let settings = app.manifest().settings();
        let mut writes: Vec<(String, AppSettingWrite)> = Vec::new();

        for (key, value) in values {
            let Some(setting) = settings.iter().find(|s| s.key().as_str() == key) else {
                return Err(AppAccessError::Denied);
            };
            match setting.kind() {
                AppSettingKind::Secret => {
                    // `null` clears; an absent key or an empty string keeps the
                    // current secret, so a form never has to ship it back.
                    if value.is_null() {
                        if setting.is_required() {
                            // Never commit a clear that the required-check would
                            // reject afterwards: the failure must not destroy the
                            // current value.
                            return Err(AppAccessError::Denied);
                        }
                        writes.push((key.clone(), AppSettingWrite::Secret(Vec::new())));
                        continue;
                    }
                    let Some(secret) = value.as_str() else {
                        return Err(AppAccessError::Denied);
                    };
                    if secret.is_empty() {
                        continue;
                    }
                    if secret.len() > MAX_SECRET_BYTES || secret.chars().any(char::is_control) {
                        return Err(AppAccessError::Denied);
                    }
                    writes.push((
                        key.clone(),
                        AppSettingWrite::Secret(secret.as_bytes().to_vec()),
                    ));
                }
                AppSettingKind::Text => {
                    let Some(text) = value.as_str() else {
                        return Err(AppAccessError::Denied);
                    };
                    if text.len() > MAX_SETTING_BYTES
                        || (setting.is_required() && text.trim().is_empty())
                    {
                        return Err(AppAccessError::Denied);
                    }
                    writes.push((
                        key.clone(),
                        AppSettingWrite::Plain(
                            serde_json::to_string(text).map_err(|_| AppAccessError::Denied)?,
                        ),
                    ));
                }
                AppSettingKind::Number => {
                    let Some(number) = value.as_f64() else {
                        return Err(AppAccessError::Denied);
                    };
                    if !number.is_finite() {
                        return Err(AppAccessError::Denied);
                    }
                    writes.push((
                        key.clone(),
                        AppSettingWrite::Plain(
                            serde_json::to_string(&number).map_err(|_| AppAccessError::Denied)?,
                        ),
                    ));
                }
                AppSettingKind::Boolean => {
                    let Some(boolean) = value.as_bool() else {
                        return Err(AppAccessError::Denied);
                    };
                    writes.push((
                        key.clone(),
                        AppSettingWrite::Plain(
                            serde_json::to_string(&boolean).map_err(|_| AppAccessError::Denied)?,
                        ),
                    ));
                }
                AppSettingKind::Select { options } => {
                    let Some(choice) = value.as_str() else {
                        return Err(AppAccessError::Denied);
                    };
                    if !options.iter().any(|option| option.value() == choice) {
                        return Err(AppAccessError::Denied);
                    }
                    writes.push((
                        key.clone(),
                        AppSettingWrite::Plain(
                            serde_json::to_string(choice).map_err(|_| AppAccessError::Denied)?,
                        ),
                    ));
                }
            }
        }

        // Write all validated values, then prove every required setting is
        // satisfied. Validation happened first, so a failure here is storage.
        for (key, value) in writes {
            store_error(store.set(&installation.to_string(), &key, value))?;
        }

        let after = store_error(store.load(&installation.to_string()))?;
        for setting in settings.iter().filter(|setting| setting.is_required()) {
            let key = setting.key().as_str();
            let present = match setting.kind() {
                AppSettingKind::Secret => after
                    .iter()
                    .any(|stored| stored.key == key && stored.secret),
                _ => after.iter().any(|stored| {
                    stored.key == key
                        && !stored.secret
                        && stored.value_text.as_deref().is_some_and(|text| {
                            serde_json::from_str::<serde_json::Value>(text)
                                .map(|value| !value.is_null())
                                .unwrap_or(false)
                        })
                }),
            };
            if !present {
                return Err(AppAccessError::Denied);
            }
        }
        Ok(())
    }

    fn session_network_origins(
        &self,
        session: SessionId,
        installation: InstallationId,
    ) -> Vec<(String, Option<u64>, bool)> {
        let now = now_seconds();
        let mut sessions = self
            .network_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(entries) = sessions.get_mut(&(session, installation)) else {
            return Vec::new();
        };
        entries.retain(|(_, expires, _)| expires.is_none_or(|expires| expires > now));
        entries.clone()
    }

    fn session_network_scope(
        &self,
        session: SessionId,
        installation: InstallationId,
        origin: &str,
    ) -> Option<bool> {
        self.session_network_origins(session, installation)
            .into_iter()
            .find(|(candidate, _, _)| candidate == origin)
            .map(|(_, _, allow_private)| allow_private)
    }

    pub(crate) fn app_network_origins_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<Vec<AppNetworkOrigin>, AppAccessError> {
        let app = self.app(identity, installation)?;
        if !has_network_permission(&app) {
            return Ok(Vec::new());
        }
        let store = self.network_store()?;
        let mut origins: HashMap<String, AppNetworkOrigin> = HashMap::new();
        for grant in
            store_error(store.list(&installation.to_string(), &identity.user_id.to_string()))?
        {
            origins.insert(
                grant.origin.clone(),
                AppNetworkOrigin {
                    origin: grant.origin,
                    validity: "durable".into(),
                    expires_at: grant.expires_at,
                    allow_private: grant.allow_private,
                },
            );
        }
        for (origin, expires_at, allow_private) in
            self.session_network_origins(identity.session_id, installation)
        {
            origins.entry(origin.clone()).or_insert(AppNetworkOrigin {
                origin,
                validity: "session".into(),
                expires_at,
                allow_private,
            });
        }
        let mut values: Vec<_> = origins.into_values().collect();
        values.sort_by(|a, b| a.origin.cmp(&b.origin));
        Ok(values)
    }

    pub(crate) fn grant_app_network_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        origin: &str,
        validity: GrantValidityChoice,
        allow_private: bool,
    ) -> Result<AppNetworkOrigin, AppAccessError> {
        let app = self.app(identity, installation)?;
        if !has_network_permission(&app) {
            return Err(AppAccessError::Denied);
        }
        let origin = consent_origin(origin).ok_or(AppAccessError::Denied)?;
        let expires_at = match validity {
            GrantValidityChoice::Session => {
                let mut sessions = self
                    .network_sessions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let entry = sessions
                    .entry((identity.session_id, installation))
                    .or_default();
                if let Some(item) = entry
                    .iter_mut()
                    .find(|(candidate, _, _)| candidate == &origin)
                {
                    *item = (origin.clone(), None, allow_private);
                } else {
                    if entry.len() >= 64 {
                        return Err(AppAccessError::Unavailable);
                    }
                    entry.push((origin.clone(), None, allow_private));
                }
                return Ok(AppNetworkOrigin {
                    origin,
                    validity: "session".into(),
                    expires_at: None,
                    allow_private,
                });
            }
            GrantValidityChoice::Durable { expires_in } => match expires_in {
                Some(seconds) if seconds == 0 || seconds > MAX_NETWORK_CONSENT_SECONDS => {
                    return Err(AppAccessError::Denied);
                }
                Some(seconds) => Some(
                    now_seconds()
                        .checked_add(seconds)
                        .ok_or(AppAccessError::Denied)?,
                ),
                None => None,
            },
        };
        let store = self.network_store()?;
        store_error(store.grant(
            &installation.to_string(),
            &identity.user_id.to_string(),
            &origin,
            expires_at,
            allow_private,
        ))?;
        Ok(AppNetworkOrigin {
            origin,
            validity: "durable".into(),
            expires_at,
            allow_private,
        })
    }

    pub(crate) fn revoke_app_network_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        origin: &str,
    ) -> Result<(), AppAccessError> {
        let app = self.app(identity, installation)?;
        if !has_network_permission(&app) {
            return Err(AppAccessError::Denied);
        }
        let origin = consent_origin(origin).ok_or(AppAccessError::Denied)?;
        {
            let mut sessions = self
                .network_sessions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(entries) = sessions.get_mut(&(identity.session_id, installation)) {
                entries.retain(|(candidate, _, _)| candidate != &origin);
            }
        }
        let store = self.network_store()?;
        store_error(store.revoke(
            &installation.to_string(),
            &identity.user_id.to_string(),
            &origin,
        ))
    }

    pub(crate) fn prepare_app_network_impl(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        request: &AppNetworkRequest,
    ) -> Result<PreparedNetworkRequest, AppNetworkError> {
        let app = self
            .app(identity, installation)
            .map_err(|_| AppNetworkError::Denied)?;
        if !has_network_permission(&app) {
            return Err(AppNetworkError::Denied);
        }
        let url = Url::parse(&request.url).map_err(|_| AppNetworkError::Blocked)?;
        if url.host_str().is_none() {
            return Err(AppNetworkError::Blocked);
        }
        let origin = request_origin(&url).ok_or(AppNetworkError::Blocked)?;
        let allow_private =
            match self.session_network_scope(identity.session_id, installation, &origin) {
                Some(scope) => scope,
                None => {
                    let store = self
                        .network_store()
                        .map_err(|_| AppNetworkError::Unavailable)?;
                    match store
                        .grant_scope(
                            &installation.to_string(),
                            &identity.user_id.to_string(),
                            &origin,
                        )
                        .map_err(|_| AppNetworkError::Unavailable)?
                    {
                        Some(scope) => scope,
                        None => return Err(AppNetworkError::ConsentRequired { origin }),
                    }
                }
            };
        if let Some(host) = url.host_str()
            && let Ok(ip) = host.parse::<IpAddr>()
        {
            if blocked(ip) {
                return Err(AppNetworkError::Blocked);
            }
            if private(ip) && !allow_private {
                return Err(AppNetworkError::PrivateConsentRequired { origin });
            }
        }
        let method = request.method.trim().to_ascii_uppercase();
        if !matches!(
            method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
        ) {
            return Err(AppNetworkError::Blocked);
        }
        let mut headers = Vec::with_capacity(request.headers.len());
        let mut seen = HashSet::new();
        for (name, value) in &request.headers {
            let name = name.trim().to_ascii_lowercase();
            if !ALLOWED_HEADERS.contains(&name.as_str()) || !seen.insert(name.clone()) {
                return Err(AppNetworkError::Blocked);
            }
            if value.len() > 1024 || value.chars().any(char::is_control) {
                return Err(AppNetworkError::Blocked);
            }
            headers.push((name, value.clone()));
        }
        let body = request.body.clone();
        if body
            .as_ref()
            .is_some_and(|body| body.len() > MAX_REQUEST_BODY_BYTES)
        {
            return Err(AppNetworkError::Blocked);
        }
        if body.is_some() && matches!(method.as_str(), "GET" | "HEAD") {
            return Err(AppNetworkError::Blocked);
        }
        if let Some(key) = &request.secret {
            let declared = app
                .manifest()
                .settings()
                .iter()
                .find(|setting| setting.key().as_str() == key)
                .is_some_and(|setting| matches!(setting.kind(), AppSettingKind::Secret));
            if !declared {
                return Err(AppNetworkError::Blocked);
            }
            let store = self
                .settings_store()
                .map_err(|_| AppNetworkError::Unavailable)?;
            let secret = store
                .secret(&installation.to_string(), key)
                .map_err(|_| AppNetworkError::Unavailable)?
                .ok_or(AppNetworkError::Blocked)?;
            let token = std::str::from_utf8(secret.as_slice())
                .map_err(|_| AppNetworkError::Blocked)?
                .trim();
            if token.is_empty() || token.chars().any(char::is_control) {
                return Err(AppNetworkError::Blocked);
            }
            headers.push(("authorization".into(), format!("Bearer {token}")));
        }
        let max_bytes = if request.max_bytes == 0 {
            DEFAULT_RESPONSE_BYTES
        } else {
            request.max_bytes.clamp(1024, MAX_RESPONSE_BYTES)
        };
        Ok(PreparedNetworkRequest {
            url: url.to_string(),
            method,
            headers,
            body,
            max_bytes,
            allow_private,
        })
    }

    /// Clears session-only network consents when the browser session ends.
    pub(crate) fn end_network_session(&self, session: SessionId) {
        let mut sessions = self
            .network_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sessions.retain(|(candidate, _), _| *candidate != session);
    }

    /// Removes every stored setting and network consent for an installation.
    /// Called after a successful uninstall; a reinstall gets a new id.
    pub(crate) fn clear_app_configuration(&self, installation: &InstallationId) {
        let key = installation.to_string();
        if let Some(store) = &self.settings {
            let _ = store.remove_for_installation(&key);
        }
        if let Some(store) = &self.network {
            let _ = store.remove_for_installation(&key);
        }
        let mut sessions = self
            .network_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sessions.retain(|(_, candidate), _| candidate != installation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::network::allowed;

    #[test]
    fn origins_are_exactly_what_the_user_approves() {
        for (value, expected) in [
            (
                "http://homeassistant.local:8123",
                "http://homeassistant.local:8123",
            ),
            ("https://ha.example", "https://ha.example"),
            ("http://192.168.1.10:8123/", "http://192.168.1.10:8123"),
        ] {
            assert_eq!(
                request_origin(&Url::parse(value).unwrap()).as_deref(),
                Some(expected)
            );
            assert_eq!(consent_origin(value).as_deref(), Some(expected));
        }
    }

    #[test]
    fn origins_reject_paths_queries_credentials_and_other_schemes() {
        for value in [
            "http://ha.local/api/states",
            "http://ha.local?token=x",
            "http://user:pass@ha.local",
            "ftp://ha.local",
            "file:///etc/passwd",
        ] {
            assert!(
                consent_origin(value).is_none(),
                "{value} must not be an origin"
            );
        }
        assert!(request_origin(&Url::parse("http://ha.local/x?y").unwrap()).is_some());
        assert!(request_origin(&Url::parse("http://ha.local/#fragment").unwrap()).is_none());
    }

    #[test]
    fn link_local_and_multicast_targets_are_blocked() {
        for value in [
            "169.254.169.254",
            "fe80::1",
            "0.0.0.0",
            "224.0.0.1",
            "ff02::1",
        ] {
            assert!(blocked(value.parse().unwrap()), "{value} must be blocked");
        }
        for value in ["127.0.0.1", "::1", "192.168.1.2", "10.0.0.1", "2001:db8::1"] {
            assert!(
                !blocked(value.parse().unwrap()),
                "{value} must not be blocked"
            );
        }
        for value in ["127.0.0.1", "::1", "192.168.1.2", "10.0.0.1", "fd00::1"] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(private(ip), "{value} must be private");
            assert!(!allowed(ip, false), "{value} must need consent");
            assert!(allowed(ip, true), "{value} must be reachable with consent");
        }
    }
}
