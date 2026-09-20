use std::error::Error;
use std::fmt;

use rumahl_core::{
    AccountStateRepository, SessionId, UnixTimestamp, UnixTimestampError, UserIdentity,
};

use crate::{AuthenticatedPrincipal, RequestAuthenticator};

pub trait SessionCredentialResolver {
    type Credential: ?Sized;
    type Error: Error + Send + Sync + 'static;

    fn resolve_session(&self, credential: &Self::Credential) -> Result<SessionId, Self::Error>;
}

pub trait AuthenticationClock {
    type Error: Error + Send + Sync + 'static;

    fn now(&self) -> Result<UnixTimestamp, Self::Error>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemAuthenticationClock;

impl AuthenticationClock for SystemAuthenticationClock {
    type Error = UnixTimestampError;

    fn now(&self) -> Result<UnixTimestamp, Self::Error> {
        UnixTimestamp::now()
    }
}

#[derive(Debug)]
pub struct LocalSessionAuthenticator<R, S, C = SystemAuthenticationClock> {
    resolver: R,
    accounts: S,
    clock: C,
}

#[derive(Debug)]
pub enum LocalSessionAuthenticationError<ResolverError, RepositoryError, ClockError> {
    Credential(ResolverError),
    Repository(RepositoryError),
    Clock(ClockError),
    InvalidSession,
}

impl<R, S> LocalSessionAuthenticator<R, S, SystemAuthenticationClock> {
    pub fn new(resolver: R, accounts: S) -> Self {
        Self {
            resolver,
            accounts,
            clock: SystemAuthenticationClock,
        }
    }
}

impl<R, S, C> LocalSessionAuthenticator<R, S, C> {
    pub fn with_clock(resolver: R, accounts: S, clock: C) -> Self {
        Self {
            resolver,
            accounts,
            clock,
        }
    }

    pub fn resolver(&self) -> &R {
        &self.resolver
    }

    pub fn account_repository(&self) -> &S {
        &self.accounts
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }
}

impl<R, S, C> RequestAuthenticator for LocalSessionAuthenticator<R, S, C>
where
    R: SessionCredentialResolver,
    S: AccountStateRepository,
    C: AuthenticationClock,
{
    type Credential = R::Credential;
    type Error = LocalSessionAuthenticationError<R::Error, S::Error, C::Error>;

    fn authenticate(
        &self,
        credential: &Self::Credential,
    ) -> Result<AuthenticatedPrincipal, Self::Error> {
        let session_id = self
            .resolver
            .resolve_session(credential)
            .map_err(LocalSessionAuthenticationError::Credential)?;
        let state = self
            .accounts
            .load()
            .map_err(LocalSessionAuthenticationError::Repository)?;
        let now = self
            .clock
            .now()
            .map_err(LocalSessionAuthenticationError::Clock)?;
        let session = state
            .sessions()
            .get(&session_id)
            .ok_or(LocalSessionAuthenticationError::InvalidSession)?;
        let account = state
            .accounts()
            .get(session.user_id())
            .ok_or(LocalSessionAuthenticationError::InvalidSession)?;

        if !account.can_authenticate() || !session.is_active_at(now) {
            return Err(LocalSessionAuthenticationError::InvalidSession);
        }

        Ok(AuthenticatedPrincipal::user(
            UserIdentity::new(*account.user_id()),
            session_id,
        ))
    }
}

impl<ResolverError, RepositoryError, ClockError> fmt::Display
    for LocalSessionAuthenticationError<ResolverError, RepositoryError, ClockError>
where
    ResolverError: Error,
    RepositoryError: Error,
    ClockError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Credential(_) | Self::InvalidSession => {
                write!(f, "local session credential is invalid")
            }
            Self::Repository(_) => write!(f, "local account state is unavailable"),
            Self::Clock(_) => write!(f, "authentication clock is unavailable"),
        }
    }
}

impl<ResolverError, RepositoryError, ClockError> Error
    for LocalSessionAuthenticationError<ResolverError, RepositoryError, ClockError>
where
    ResolverError: Error + 'static,
    RepositoryError: Error + 'static,
    ClockError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Credential(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::InvalidSession => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use rumahl_core::{AccountState, AccountStateRepository, UnixTimestamp};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct UnknownCredential;

    impl fmt::Display for UnknownCredential {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "unknown credential")
        }
    }

    impl Error for UnknownCredential {}

    struct OpaqueCredentialResolver {
        credential: String,
        session_id: SessionId,
    }

    impl SessionCredentialResolver for OpaqueCredentialResolver {
        type Credential = str;
        type Error = UnknownCredential;

        fn resolve_session(&self, credential: &Self::Credential) -> Result<SessionId, Self::Error> {
            if credential == self.credential {
                Ok(self.session_id)
            } else {
                Err(UnknownCredential)
            }
        }
    }

    #[derive(Clone)]
    struct MemoryAccountRepository {
        state: AccountState,
    }

    impl AccountStateRepository for MemoryAccountRepository {
        type Error = Infallible;

        fn load(&self) -> Result<AccountState, Self::Error> {
            Ok(self.state.clone())
        }

        fn store(&self, _state: &AccountState) -> Result<(), Self::Error> {
            unreachable!("authentication only reads the current account state")
        }
    }

    #[derive(Clone, Copy)]
    struct FixedClock(UnixTimestamp);

    impl AuthenticationClock for FixedClock {
        type Error = Infallible;

        fn now(&self) -> Result<UnixTimestamp, Self::Error> {
            Ok(self.0)
        }
    }

    fn authenticator(
        now: u64,
    ) -> (
        LocalSessionAuthenticator<OpaqueCredentialResolver, MemoryAccountRepository, FixedClock>,
        rumahl_core::UserId,
        SessionId,
    ) {
        let mut state = AccountState::new();
        let user_id = state.create_account("kai", "Kai").unwrap();
        let session_id = state
            .start_session(
                &user_id,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        (
            LocalSessionAuthenticator::with_clock(
                OpaqueCredentialResolver {
                    credential: "opaque-browser-session".to_owned(),
                    session_id,
                },
                MemoryAccountRepository { state },
                FixedClock(UnixTimestamp::from_seconds(now)),
            ),
            user_id,
            session_id,
        )
    }

    #[test]
    fn resolves_opaque_credential_and_revalidates_os_session() {
        let (authenticator, user_id, session_id) = authenticator(150);

        let principal = authenticator
            .authenticate("opaque-browser-session")
            .unwrap();

        assert_eq!(
            principal,
            AuthenticatedPrincipal::user(UserIdentity::new(user_id), session_id)
        );
    }

    #[test]
    fn rejects_unknown_opaque_credential() {
        let (authenticator, _, _) = authenticator(150);

        assert!(matches!(
            authenticator.authenticate("a-session-id-is-not-a-token"),
            Err(LocalSessionAuthenticationError::Credential(
                UnknownCredential
            ))
        ));
    }

    #[test]
    fn rejects_expired_session_after_credential_resolution() {
        let (authenticator, _, _) = authenticator(200);

        assert!(matches!(
            authenticator.authenticate("opaque-browser-session"),
            Err(LocalSessionAuthenticationError::InvalidSession)
        ));
    }

    #[test]
    fn rejects_revoked_session_after_credential_resolution() {
        let (mut authenticator, user_id, _) = authenticator(150);
        authenticator
            .accounts
            .state
            .lock_account(&user_id, UnixTimestamp::from_seconds(140))
            .unwrap();

        assert!(matches!(
            authenticator.authenticate("opaque-browser-session"),
            Err(LocalSessionAuthenticationError::InvalidSession)
        ));
    }
}
