use std::error::Error;
use std::fmt;

use crate::{SessionId, UserId};

use super::{AccountSession, AccountSessionError, UnixTimestamp};

#[derive(Debug, Default, Clone)]
pub struct AccountSessionRegistry {
    sessions: Vec<AccountSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountSessionRegistryError {
    SessionAlreadyRegistered,
    InvalidSession(AccountSessionError),
}

impl AccountSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, session: AccountSession) -> Result<(), AccountSessionRegistryError> {
        if self.get(session.session_id()).is_some() {
            return Err(AccountSessionRegistryError::SessionAlreadyRegistered);
        }

        self.sessions.push(session);

        Ok(())
    }

    pub fn get(&self, session_id: &SessionId) -> Option<&AccountSession> {
        self.sessions
            .iter()
            .find(|session| session.session_id() == session_id)
    }

    pub fn sessions_for_user(&self, user_id: &UserId) -> impl Iterator<Item = &AccountSession> {
        self.sessions
            .iter()
            .filter(move |session| session.user_id() == user_id)
    }

    pub fn sessions(&self) -> &[AccountSession] {
        &self.sessions
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub(crate) fn revoke_for_user(
        &mut self,
        user_id: &UserId,
        timestamp: UnixTimestamp,
    ) -> Result<usize, AccountSessionRegistryError> {
        let mut revoked = 0;

        for session in self
            .sessions
            .iter_mut()
            .filter(|session| session.user_id() == user_id)
        {
            if session
                .revoke(timestamp)
                .map_err(AccountSessionRegistryError::InvalidSession)?
            {
                revoked += 1;
            }
        }

        Ok(revoked)
    }
}

impl fmt::Display for AccountSessionRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionAlreadyRegistered => write!(f, "account session is already registered"),
            Self::InvalidSession(error) => write!(f, "account session is invalid: {error}"),
        }
    }
}

impl Error for AccountSessionRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SessionAlreadyRegistered => None,
            Self::InvalidSession(error) => Some(error),
        }
    }
}
