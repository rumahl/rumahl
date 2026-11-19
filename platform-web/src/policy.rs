//! Policy enforcement points and the administration API for assignments.
//!
//! [`PolicyEngine`] is the web layer's view of the policy store: one question
//! per enforcement point, each with a safe default so a deployment without an
//! engine keeps today's behavior instead of failing closed or open by
//! accident. The engine is resolved per request, never cached, so an
//! assignment applies to the very next request.
use std::sync::Arc;

use rumahl_core::{
    DEFAULT_APP_STORAGE_QUOTA_BYTES, DEFAULT_MAX_FILE_BYTES, DEFAULT_PASSWORD_ATTEMPT_LIMIT,
    DEFAULT_PASSWORD_LOCKOUT_SECONDS, DEFAULT_PASSWORD_MIN_LENGTH, DEFAULT_PERSONAL_QUOTA_BYTES,
    UserId,
};

/// The password rules a device enforces (device-wide, like a domain policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordRules {
    pub minimum_characters: u64,
    /// 0–4 of the four classes (lowercase, uppercase, digits, symbols).
    pub required_character_classes: u64,
    pub attempt_limit: u64,
    pub lockout_seconds: u64,
}

impl Default for PasswordRules {
    fn default() -> Self {
        Self {
            minimum_characters: DEFAULT_PASSWORD_MIN_LENGTH,
            required_character_classes: 0,
            attempt_limit: DEFAULT_PASSWORD_ATTEMPT_LIMIT,
            lockout_seconds: DEFAULT_PASSWORD_LOCKOUT_SECONDS,
        }
    }
}

/// Policy questions the platform web layer asks on every enforcement point.
///
/// Defaults mirror the catalog: without an engine, advanced mode stays
/// available, cross-user home access stays off, and the budgets and lifetimes
/// keep their built-in values.
pub trait PolicyEngine: Send + Sync {
    /// `rumahl.os.advanced_mode`: false forbids raising the exposure mode and
    /// closes the apps/system areas even for a stale stored advanced mode.
    fn advanced_mode_allowed(&self, _user: UserId) -> bool {
        true
    }

    /// `rumahl.os.allow_store_install`: false restricts package imports to
    /// administrators.
    fn store_install_allowed(&self, _user: UserId) -> bool {
        true
    }

    /// `rumahl.os.allow_app_uninstall`: false restricts uninstalls to
    /// administrators.
    fn app_uninstall_allowed(&self, _user: UserId) -> bool {
        true
    }

    /// `rumahl.home.cross_user_read`: administrators may open other accounts'
    /// homes read-only. The engine answers for the *actor*; the role check
    /// happens beside it, so policy and role both have to allow.
    fn home_cross_user_read(&self, _actor: UserId) -> bool {
        false
    }

    /// `rumahl.files.quota_bytes`: the personal-files budget of one account.
    fn personal_file_quota(&self, _user: UserId) -> u64 {
        DEFAULT_PERSONAL_QUOTA_BYTES
    }

    /// `rumahl.files.max_file_bytes`: the largest single file an account may
    /// store through the shell.
    fn max_file_bytes(&self, _user: UserId) -> u64 {
        DEFAULT_MAX_FILE_BYTES
    }

    /// `rumahl.files.durable_grants`: false limits app consent to the current
    /// session, the strongest form a policy can force.
    fn durable_grants_allowed(&self, _user: UserId) -> bool {
        true
    }

    /// `rumahl.session.lifetime_seconds`: how long a browser session stays
    /// valid for one account.
    fn session_lifetime_seconds(&self, _user: UserId) -> u64 {
        crate::SESSION_SECONDS
    }

    /// `rumahl.password.*` (device-wide): the rules every credential follows.
    fn password_rules(&self) -> PasswordRules {
        PasswordRules::default()
    }

    /// `rumahl.apps.storage_quota_bytes` (device-wide): the private data
    /// budget per installed app.
    fn app_storage_quota(&self) -> u64 {
        DEFAULT_APP_STORAGE_QUOTA_BYTES
    }

    /// `rumahl.apps.allowed_publishers` (device-wide): the publishers whose
    /// packages may be installed. Empty means every trusted publisher.
    fn allowed_publishers(&self) -> Vec<String> {
        Vec::new()
    }
}

impl<T: PolicyEngine + ?Sized> PolicyEngine for Arc<T> {
    fn advanced_mode_allowed(&self, user: UserId) -> bool {
        (**self).advanced_mode_allowed(user)
    }
    fn store_install_allowed(&self, user: UserId) -> bool {
        (**self).store_install_allowed(user)
    }
    fn app_uninstall_allowed(&self, user: UserId) -> bool {
        (**self).app_uninstall_allowed(user)
    }
    fn home_cross_user_read(&self, actor: UserId) -> bool {
        (**self).home_cross_user_read(actor)
    }
    fn personal_file_quota(&self, user: UserId) -> u64 {
        (**self).personal_file_quota(user)
    }
    fn max_file_bytes(&self, user: UserId) -> u64 {
        (**self).max_file_bytes(user)
    }
    fn durable_grants_allowed(&self, user: UserId) -> bool {
        (**self).durable_grants_allowed(user)
    }
    fn session_lifetime_seconds(&self, user: UserId) -> u64 {
        (**self).session_lifetime_seconds(user)
    }
    fn password_rules(&self) -> PasswordRules {
        (**self).password_rules()
    }
    fn app_storage_quota(&self) -> u64 {
        (**self).app_storage_quota()
    }
    fn allowed_publishers(&self) -> Vec<String> {
        (**self).allowed_publishers()
    }
}
