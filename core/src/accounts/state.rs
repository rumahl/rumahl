use std::error::Error;
use std::fmt;

use crate::{SessionId, UserId};

use super::{
    AccountRegistry, AccountRegistryError, AccountSession, AccountSessionError,
    AccountSessionRegistry, AccountSessionRegistryError, AccountStatus, LocalAccount,
    LocalAccountError, UnixTimestamp,
};

#[derive(Debug, Default, Clone)]
pub struct AccountState {
    accounts: AccountRegistry,
    sessions: AccountSessionRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountStateError {
    InvalidAccount(LocalAccountError),
    InvalidSession(AccountSessionError),
    AccountRegistry(AccountRegistryError),
    SessionRegistry(AccountSessionRegistryError),
    AccountNotFound,
    AccountUnavailable,
    SessionReferencesUnknownAccount,
    UnavailableAccountHasLiveSession,
}

impl AccountState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn restore(
        accounts: impl IntoIterator<Item = LocalAccount>,
        sessions: impl IntoIterator<Item = AccountSession>,
    ) -> Result<Self, AccountStateError> {
        let mut state = Self::new();

        for account in accounts {
            state
                .accounts
                .register(account)
                .map_err(AccountStateError::AccountRegistry)?;
        }

        for session in sessions {
            let account = state
                .accounts
                .get(session.user_id())
                .ok_or(AccountStateError::SessionReferencesUnknownAccount)?;

            if !account.can_authenticate() && session.revoked_at().is_none() {
                return Err(AccountStateError::UnavailableAccountHasLiveSession);
            }

            state
                .sessions
                .register(session)
                .map_err(AccountStateError::SessionRegistry)?;
        }

        Ok(state)
    }

    pub fn create_account(
        &mut self,
        username: &str,
        display_name: &str,
    ) -> Result<UserId, AccountStateError> {
        let account = LocalAccount::create(username, display_name)
            .map_err(AccountStateError::InvalidAccount)?;
        let user_id = *account.user_id();

        self.accounts
            .register(account)
            .map_err(AccountStateError::AccountRegistry)?;

        Ok(user_id)
    }

    pub fn start_session(
        &mut self,
        user_id: &UserId,
        authenticated_at: UnixTimestamp,
        expires_at: UnixTimestamp,
    ) -> Result<SessionId, AccountStateError> {
        let account = self
            .accounts
            .get(user_id)
            .ok_or(AccountStateError::AccountNotFound)?;

        if !account.can_authenticate() {
            return Err(AccountStateError::AccountUnavailable);
        }

        let session = AccountSession::start(account, authenticated_at, expires_at)
            .map_err(AccountStateError::InvalidSession)?;
        let session_id = *session.session_id();

        self.sessions
            .register(session)
            .map_err(AccountStateError::SessionRegistry)?;

        Ok(session_id)
    }

    pub fn lock_account(
        &mut self,
        user_id: &UserId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, AccountStateError> {
        self.set_unavailable(user_id, AccountStatus::Locked, revoked_at)
    }

    pub fn disable_account(
        &mut self,
        user_id: &UserId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, AccountStateError> {
        self.set_unavailable(user_id, AccountStatus::Disabled, revoked_at)
    }

    pub fn unlock_account(&mut self, user_id: &UserId) -> Result<(), AccountStateError> {
        let account = self
            .accounts
            .get_mut(user_id)
            .ok_or(AccountStateError::AccountNotFound)?;

        if account.status() == AccountStatus::Disabled {
            return Err(AccountStateError::AccountUnavailable);
        }

        account.set_status(AccountStatus::Active);

        Ok(())
    }

    pub fn revoke_sessions_for_user(
        &mut self,
        user_id: &UserId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, AccountStateError> {
        if self.accounts.get(user_id).is_none() {
            return Err(AccountStateError::AccountNotFound);
        }

        let mut staged = self.clone();
        let revoked = staged
            .sessions
            .revoke_for_user(user_id, revoked_at)
            .map_err(AccountStateError::SessionRegistry)?;

        *self = staged;

        Ok(revoked)
    }

    pub fn accounts(&self) -> &AccountRegistry {
        &self.accounts
    }

    pub fn sessions(&self) -> &AccountSessionRegistry {
        &self.sessions
    }

    fn set_unavailable(
        &mut self,
        user_id: &UserId,
        status: AccountStatus,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, AccountStateError> {
        if self.accounts.get(user_id).is_none() {
            return Err(AccountStateError::AccountNotFound);
        }

        let mut staged = self.clone();

        staged
            .accounts
            .get_mut(user_id)
            .expect("account existence checked before staging")
            .set_status(status);
        let revoked = staged
            .sessions
            .revoke_for_user(user_id, revoked_at)
            .map_err(AccountStateError::SessionRegistry)?;

        *self = staged;

        Ok(revoked)
    }
}

impl fmt::Display for AccountStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccount(error) => write!(f, "account is invalid: {error}"),
            Self::InvalidSession(error) => write!(f, "account session is invalid: {error}"),
            Self::AccountRegistry(error) => write!(f, "account registry rejected value: {error}"),
            Self::SessionRegistry(error) => write!(f, "session registry rejected value: {error}"),
            Self::AccountNotFound => write!(f, "account was not found"),
            Self::AccountUnavailable => write!(f, "account is not available for authentication"),
            Self::SessionReferencesUnknownAccount => {
                write!(f, "account session references an unknown account")
            }
            Self::UnavailableAccountHasLiveSession => {
                write!(f, "locked or disabled account has a non-revoked session")
            }
        }
    }
}

impl Error for AccountStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAccount(error) => Some(error),
            Self::InvalidSession(error) => Some(error),
            Self::AccountRegistry(error) => Some(error),
            Self::SessionRegistry(error) => Some(error),
            Self::AccountNotFound
            | Self::AccountUnavailable
            | Self::SessionReferencesUnknownAccount
            | Self::UnavailableAccountHasLiveSession => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_multiple_accounts_with_separate_sessions() {
        let mut state = AccountState::new();
        let kai = state.create_account("kai", "Kai").unwrap();
        let lea = state.create_account("lea", "Lea").unwrap();

        let kai_session = state
            .start_session(
                &kai,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();
        let lea_session = state
            .start_session(
                &lea,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        assert_ne!(kai_session, lea_session);
        assert_eq!(state.sessions().sessions_for_user(&kai).count(), 1);
        assert_eq!(state.sessions().sessions_for_user(&lea).count(), 1);
    }

    #[test]
    fn locking_account_atomically_revokes_only_its_sessions() {
        let mut state = AccountState::new();
        let kai = state.create_account("kai", "Kai").unwrap();
        let lea = state.create_account("lea", "Lea").unwrap();
        let kai_session = state
            .start_session(
                &kai,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(300),
            )
            .unwrap();
        let lea_session = state
            .start_session(
                &lea,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(300),
            )
            .unwrap();

        assert_eq!(
            state
                .lock_account(&kai, UnixTimestamp::from_seconds(200))
                .unwrap(),
            1
        );
        assert_eq!(
            state.accounts().get(&kai).unwrap().status(),
            AccountStatus::Locked
        );
        assert!(
            state
                .sessions()
                .get(&kai_session)
                .unwrap()
                .revoked_at()
                .is_some()
        );
        assert!(
            state
                .sessions()
                .get(&lea_session)
                .unwrap()
                .revoked_at()
                .is_none()
        );
    }

    #[test]
    fn failed_revocation_does_not_partially_lock_account() {
        let mut state = AccountState::new();
        let kai = state.create_account("kai", "Kai").unwrap();
        state
            .start_session(
                &kai,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(300),
            )
            .unwrap();

        let result = state.lock_account(&kai, UnixTimestamp::from_seconds(99));

        assert!(matches!(result, Err(AccountStateError::SessionRegistry(_))));
        assert_eq!(
            state.accounts().get(&kai).unwrap().status(),
            AccountStatus::Active
        );
        assert!(
            state
                .sessions()
                .sessions_for_user(&kai)
                .all(|session| session.revoked_at().is_none())
        );
    }

    #[test]
    fn revoking_sessions_does_not_change_account_availability() {
        let mut state = AccountState::new();
        let kai = state.create_account("kai", "Kai").unwrap();
        let session_id = state
            .start_session(
                &kai,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(300),
            )
            .unwrap();

        assert_eq!(
            state
                .revoke_sessions_for_user(&kai, UnixTimestamp::from_seconds(200))
                .unwrap(),
            1
        );
        assert_eq!(
            state.accounts().get(&kai).unwrap().status(),
            AccountStatus::Active
        );
        assert_eq!(
            state.sessions().get(&session_id).unwrap().revoked_at(),
            Some(UnixTimestamp::from_seconds(200))
        );
    }
}
