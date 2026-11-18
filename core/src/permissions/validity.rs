//! Grant lifetime.
//!
//! A durable grant is part of the platform snapshot and survives restarts,
//! optionally bounded by an expiry timestamp. A session grant is bound to one
//! browser session: it is never persisted, so neither a restart nor a new
//! session can ever resurrect it. Both kinds lose every fight against the
//! authorization engine once they are no longer usable.

use crate::UnixTimestamp;
use crate::identity::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantValidity {
    /// Persists with the platform snapshot. `expires_at` bounds the grant;
    /// `None` keeps it until the user revokes it.
    Durable { expires_at: Option<UnixTimestamp> },
    /// Usable only while the operation runs in exactly this session.
    Session { session: SessionId },
}

impl GrantValidity {
    /// A durable grant without expiry: usable until the user revokes it.
    pub fn durable() -> Self {
        Self::Durable { expires_at: None }
    }

    /// A durable grant that stops working once `expires_at` is reached.
    pub fn durable_until(expires_at: UnixTimestamp) -> Self {
        Self::Durable {
            expires_at: Some(expires_at),
        }
    }

    /// A grant that dies with the given browser session.
    pub fn session(session: SessionId) -> Self {
        Self::Session { session }
    }

    /// Only durable grants belong in the platform snapshot.
    pub fn is_durable(&self) -> bool {
        matches!(self, Self::Durable { .. })
    }

    /// Expiry timestamp of a durable grant, if any.
    pub fn expires_at(&self) -> Option<UnixTimestamp> {
        match self {
            Self::Durable { expires_at } => *expires_at,
            Self::Session { .. } => None,
        }
    }

    /// A grant is usable only while it is unexpired and, for session grants,
    /// while the operation runs in exactly the bound session. Expired or
    /// unbound grants are indistinguishable from missing grants.
    pub fn is_usable(&self, session: Option<&SessionId>, now: UnixTimestamp) -> bool {
        match self {
            Self::Durable { expires_at } => expires_at.is_none_or(|expires_at| now < expires_at),
            Self::Session {
                session: bound_session,
            } => session == Some(bound_session),
        }
    }
}

impl Default for GrantValidity {
    fn default() -> Self {
        Self::durable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_grant_never_expires_on_its_own() {
        let validity = GrantValidity::durable();

        assert_eq!(validity.expires_at(), None);
        assert!(validity.is_durable());
        assert!(validity.is_usable(None, UnixTimestamp::from_seconds(1)));
        assert!(validity.is_usable(None, UnixTimestamp::from_seconds(u64::MAX)));
    }

    #[test]
    fn durable_expiry_is_exclusive() {
        let expires_at = UnixTimestamp::from_seconds(1_000);
        let validity = GrantValidity::durable_until(expires_at);

        assert_eq!(validity.expires_at(), Some(expires_at));
        assert!(validity.is_usable(None, UnixTimestamp::from_seconds(999)));
        assert!(!validity.is_usable(None, expires_at));
        assert!(!validity.is_usable(None, UnixTimestamp::from_seconds(1_001)));
    }

    #[test]
    fn session_grant_is_usable_only_in_its_session() {
        let session = SessionId::new();
        let other = SessionId::new();
        let validity = GrantValidity::session(session);

        assert!(!validity.is_durable());
        assert_eq!(validity.expires_at(), None);
        assert!(validity.is_usable(Some(&session), UnixTimestamp::from_seconds(1)));
        assert!(!validity.is_usable(Some(&other), UnixTimestamp::from_seconds(1)));
        // Without a session (background or recovered operation) a session
        // grant can never apply.
        assert!(!validity.is_usable(None, UnixTimestamp::from_seconds(1)));
    }

    #[test]
    fn default_validity_is_durable() {
        assert_eq!(GrantValidity::default(), GrantValidity::durable());
    }
}
