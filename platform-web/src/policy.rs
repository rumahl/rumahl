//! Policy enforcement points and the administration API for assignments.
//!
//! [`PolicyEngine`] is the web layer's view of the policy store: three
//! questions the enforcement points actually ask, each with a safe default so
//! a deployment without a policy engine keeps today's behavior instead of
//! failing closed or open by accident. The engine is resolved per request,
//! never cached, so an assignment applies to the very next request.
use std::sync::Arc;

use rumahl_core::{DEFAULT_PERSONAL_QUOTA_BYTES, UserId};

/// Policy questions the platform web layer asks on every enforcement point.
///
/// Defaults mirror the catalog: without an engine, advanced mode stays
/// available, cross-user home access stays off, and the personal quota is the
/// built-in 128 MiB.
pub trait PolicyEngine: Send + Sync {
    /// `rumahl.os.advanced_mode`: false forbids raising the exposure mode and
    /// closes the apps/system areas even for a stale stored advanced mode.
    fn advanced_mode_allowed(&self, _user: UserId) -> bool {
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
}

impl<T: PolicyEngine + ?Sized> PolicyEngine for Arc<T> {
    fn advanced_mode_allowed(&self, user: UserId) -> bool {
        (**self).advanced_mode_allowed(user)
    }
    fn home_cross_user_read(&self, actor: UserId) -> bool {
        (**self).home_cross_user_read(actor)
    }
    fn personal_file_quota(&self, user: UserId) -> u64 {
        (**self).personal_file_quota(user)
    }
}
