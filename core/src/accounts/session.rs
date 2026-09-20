use std::error::Error;
use std::fmt;

use crate::{SessionId, UserId};

use super::{LocalAccount, UnixTimestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSession {
    session_id: SessionId,
    user_id: UserId,
    authenticated_at: UnixTimestamp,
    last_seen_at: UnixTimestamp,
    reauthenticated_at: Option<UnixTimestamp>,
    expires_at: UnixTimestamp,
    revoked_at: Option<UnixTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountSessionError {
    AccountUnavailable,
    ExpiryNotAfterAuthentication,
    LastSeenBeforeAuthentication,
    LastSeenAtOrAfterExpiry,
    ReauthenticationBeforeAuthentication,
    ReauthenticationAfterLastSeen,
    RevocationBeforeAuthentication,
    ObservationMovedBackwards,
    SessionExpired,
    SessionRevoked,
}

impl AccountSession {
    pub fn start(
        account: &LocalAccount,
        authenticated_at: UnixTimestamp,
        expires_at: UnixTimestamp,
    ) -> Result<Self, AccountSessionError> {
        if !account.can_authenticate() {
            return Err(AccountSessionError::AccountUnavailable);
        }

        Self::restore(
            SessionId::new(),
            *account.user_id(),
            authenticated_at,
            authenticated_at,
            Some(authenticated_at),
            expires_at,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        session_id: SessionId,
        user_id: UserId,
        authenticated_at: UnixTimestamp,
        last_seen_at: UnixTimestamp,
        reauthenticated_at: Option<UnixTimestamp>,
        expires_at: UnixTimestamp,
        revoked_at: Option<UnixTimestamp>,
    ) -> Result<Self, AccountSessionError> {
        if expires_at <= authenticated_at {
            return Err(AccountSessionError::ExpiryNotAfterAuthentication);
        }

        if last_seen_at < authenticated_at {
            return Err(AccountSessionError::LastSeenBeforeAuthentication);
        }

        if last_seen_at >= expires_at {
            return Err(AccountSessionError::LastSeenAtOrAfterExpiry);
        }

        if let Some(reauthenticated_at) = reauthenticated_at {
            if reauthenticated_at < authenticated_at {
                return Err(AccountSessionError::ReauthenticationBeforeAuthentication);
            }

            if reauthenticated_at > last_seen_at {
                return Err(AccountSessionError::ReauthenticationAfterLastSeen);
            }
        }

        if revoked_at.is_some_and(|revoked_at| revoked_at < authenticated_at) {
            return Err(AccountSessionError::RevocationBeforeAuthentication);
        }

        Ok(Self {
            session_id,
            user_id,
            authenticated_at,
            last_seen_at,
            reauthenticated_at,
            expires_at,
            revoked_at,
        })
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn authenticated_at(&self) -> UnixTimestamp {
        self.authenticated_at
    }

    pub fn last_seen_at(&self) -> UnixTimestamp {
        self.last_seen_at
    }

    pub fn reauthenticated_at(&self) -> Option<UnixTimestamp> {
        self.reauthenticated_at
    }

    pub fn expires_at(&self) -> UnixTimestamp {
        self.expires_at
    }

    pub fn revoked_at(&self) -> Option<UnixTimestamp> {
        self.revoked_at
    }

    pub fn is_active_at(&self, timestamp: UnixTimestamp) -> bool {
        self.revoked_at.is_none()
            && timestamp >= self.authenticated_at
            && timestamp < self.expires_at
    }

    pub fn touch(&mut self, timestamp: UnixTimestamp) -> Result<(), AccountSessionError> {
        self.ensure_observation(timestamp)?;
        self.last_seen_at = timestamp;

        Ok(())
    }

    pub fn reauthenticate(&mut self, timestamp: UnixTimestamp) -> Result<(), AccountSessionError> {
        self.ensure_observation(timestamp)?;
        self.last_seen_at = timestamp;
        self.reauthenticated_at = Some(timestamp);

        Ok(())
    }

    pub fn revoke(&mut self, timestamp: UnixTimestamp) -> Result<bool, AccountSessionError> {
        if timestamp < self.authenticated_at {
            return Err(AccountSessionError::RevocationBeforeAuthentication);
        }

        if self.revoked_at.is_some() {
            return Ok(false);
        }

        self.revoked_at = Some(timestamp);

        Ok(true)
    }

    fn ensure_observation(&self, timestamp: UnixTimestamp) -> Result<(), AccountSessionError> {
        if self.revoked_at.is_some() {
            return Err(AccountSessionError::SessionRevoked);
        }

        if timestamp < self.last_seen_at {
            return Err(AccountSessionError::ObservationMovedBackwards);
        }

        if timestamp >= self.expires_at {
            return Err(AccountSessionError::SessionExpired);
        }

        Ok(())
    }
}

impl fmt::Display for AccountSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountUnavailable => write!(f, "account cannot start a session"),
            Self::ExpiryNotAfterAuthentication => {
                write!(f, "session expiry must be after authentication")
            }
            Self::LastSeenBeforeAuthentication => {
                write!(f, "session last-seen time cannot precede authentication")
            }
            Self::LastSeenAtOrAfterExpiry => {
                write!(f, "session last-seen time must precede expiry")
            }
            Self::ReauthenticationBeforeAuthentication => {
                write!(f, "session reauthentication cannot precede authentication")
            }
            Self::ReauthenticationAfterLastSeen => {
                write!(f, "session reauthentication cannot be after last-seen time")
            }
            Self::RevocationBeforeAuthentication => {
                write!(f, "session revocation cannot precede authentication")
            }
            Self::ObservationMovedBackwards => {
                write!(f, "session observation cannot move backwards")
            }
            Self::SessionExpired => write!(f, "session has expired"),
            Self::SessionRevoked => write!(f, "session has been revoked"),
        }
    }
}

impl Error for AccountSessionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> LocalAccount {
        LocalAccount::create("kai", "Kai").unwrap()
    }

    #[test]
    fn starts_active_session_for_account() {
        let session = AccountSession::start(
            &account(),
            UnixTimestamp::from_seconds(100),
            UnixTimestamp::from_seconds(200),
        )
        .unwrap();

        assert!(session.is_active_at(UnixTimestamp::from_seconds(150)));
        assert!(!session.is_active_at(UnixTimestamp::from_seconds(200)));
    }

    #[test]
    fn rejects_invalid_time_ordering() {
        assert_eq!(
            AccountSession::start(
                &account(),
                UnixTimestamp::from_seconds(200),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap_err(),
            AccountSessionError::ExpiryNotAfterAuthentication
        );
    }

    #[test]
    fn revocation_is_idempotent() {
        let mut session = AccountSession::start(
            &account(),
            UnixTimestamp::from_seconds(100),
            UnixTimestamp::from_seconds(200),
        )
        .unwrap();

        assert!(session.revoke(UnixTimestamp::from_seconds(150)).unwrap());
        assert!(!session.revoke(UnixTimestamp::from_seconds(160)).unwrap());
        assert_eq!(session.revoked_at(), Some(UnixTimestamp::from_seconds(150)));
    }
}
