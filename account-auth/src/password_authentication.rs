use std::error::Error;
use std::fmt;

use rumahl_core::{
    AccountState, AccountStateError, AccountStatus, AccountUsername, SessionId, UnixTimestamp,
    UserId,
};

use crate::{
    Argon2idPasswordEngine, Password, PasswordAttempt, PasswordAttemptPolicy, PasswordBlocklist,
    PasswordCredentialRecord, PasswordCredentialRepository, PasswordEngineError, PasswordPolicy,
    PasswordPolicyError,
};

#[derive(Debug)]
pub struct PasswordAuthenticationService<R, B> {
    repository: R,
    blocklist: B,
    password_policy: PasswordPolicy,
    attempt_policy: PasswordAttemptPolicy,
    engine: Argon2idPasswordEngine,
    dummy_hash: crate::PasswordHashRecord,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedLocalAccount {
    user_id: UserId,
    authenticated_at: UnixTimestamp,
    password_hash: crate::PasswordHashRecord,
}

#[derive(Debug)]
pub enum PasswordAuthenticationServiceInitializationError {
    Password(PasswordPolicyError),
    Engine(PasswordEngineError),
}

#[derive(Debug)]
pub enum PasswordEnrollmentError<RepositoryError> {
    AccountNotFound,
    AccountDisabled,
    Policy(PasswordPolicyError),
    Engine(PasswordEngineError),
    Repository(RepositoryError),
}

#[derive(Debug)]
pub enum PasswordAuthenticationError<RepositoryError> {
    InvalidCredentials,
    Engine(PasswordEngineError),
    Repository(RepositoryError),
}

impl<R, B> PasswordAuthenticationService<R, B>
where
    R: PasswordCredentialRepository,
    B: PasswordBlocklist,
{
    pub fn new(
        repository: R,
        blocklist: B,
    ) -> Result<Self, PasswordAuthenticationServiceInitializationError> {
        Self::with_policies(
            repository,
            blocklist,
            PasswordPolicy::default(),
            PasswordAttemptPolicy::default(),
        )
    }

    pub fn with_policies(
        repository: R,
        blocklist: B,
        password_policy: PasswordPolicy,
        attempt_policy: PasswordAttemptPolicy,
    ) -> Result<Self, PasswordAuthenticationServiceInitializationError> {
        let engine = Argon2idPasswordEngine::new();
        let dummy_password =
            Password::prepare("rumahl constant-time unknown account verifier".to_owned())
                .map_err(PasswordAuthenticationServiceInitializationError::Password)?;
        let dummy_hash = engine
            .hash(&dummy_password)
            .map_err(PasswordAuthenticationServiceInitializationError::Engine)?;

        Ok(Self {
            repository,
            blocklist,
            password_policy,
            attempt_policy,
            engine,
            dummy_hash,
        })
    }

    /// Enrolls a password for an existing account at the credential-adapter
    /// level. User-initiated password changes must go through
    /// `LocalAccountAdministrationService::change_password` so session
    /// revocation and credential replacement share one persistence transaction.
    pub fn set_password(
        &self,
        state: &AccountState,
        user_id: &UserId,
        candidate: String,
        changed_at: UnixTimestamp,
    ) -> Result<(), PasswordEnrollmentError<R::Error>> {
        let account = state
            .accounts()
            .get(user_id)
            .ok_or(PasswordEnrollmentError::AccountNotFound)?;

        if account.status() == AccountStatus::Disabled {
            return Err(PasswordEnrollmentError::AccountDisabled);
        }

        let password = Password::prepare(candidate).map_err(PasswordEnrollmentError::Policy)?;
        self.password_policy
            .validate(&password, &self.blocklist)
            .map_err(PasswordEnrollmentError::Policy)?;
        let password_hash = self
            .engine
            .hash(&password)
            .map_err(PasswordEnrollmentError::Engine)?;

        self.repository
            .replace(&PasswordCredentialRecord::new(
                *user_id,
                password_hash,
                changed_at,
            ))
            .map_err(PasswordEnrollmentError::Repository)
    }

    pub fn authenticate(
        &self,
        state: &AccountState,
        username: &str,
        candidate: String,
        attempted_at: UnixTimestamp,
    ) -> Result<VerifiedLocalAccount, PasswordAuthenticationError<R::Error>> {
        let password = Password::prepare(candidate)
            .map_err(|_| PasswordAuthenticationError::InvalidCredentials)?;
        let account = AccountUsername::parse(username)
            .ok()
            .and_then(|username| state.accounts().get_by_username(&username));

        let Some(account) = account else {
            self.verify_dummy(&password)?;
            return Err(PasswordAuthenticationError::InvalidCredentials);
        };

        let attempt = self
            .repository
            .begin_attempt(account.user_id(), attempted_at, self.attempt_policy)
            .map_err(PasswordAuthenticationError::Repository)?;

        let PasswordAttempt::Allowed(credential) = attempt else {
            self.verify_dummy(&password)?;
            return Err(PasswordAuthenticationError::InvalidCredentials);
        };

        let valid = self
            .engine
            .verify(&password, credential.password_hash())
            .map_err(PasswordAuthenticationError::Engine)?;

        if !valid || !account.can_authenticate() {
            return Err(PasswordAuthenticationError::InvalidCredentials);
        }

        self.repository
            .record_success(account.user_id())
            .map_err(PasswordAuthenticationError::Repository)?;

        Ok(VerifiedLocalAccount {
            user_id: *account.user_id(),
            authenticated_at: attempted_at,
            password_hash: credential.password_hash().clone(),
        })
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    fn verify_dummy(
        &self,
        password: &Password,
    ) -> Result<(), PasswordAuthenticationError<R::Error>> {
        self.engine
            .verify(password, &self.dummy_hash)
            .map(|_| ())
            .map_err(PasswordAuthenticationError::Engine)
    }
}

impl VerifiedLocalAccount {
    /// Compare inside the session-creation transaction to reject a proof made
    /// stale by a concurrent password reset.
    pub fn password_hash(&self) -> &crate::PasswordHashRecord {
        &self.password_hash
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn authenticated_at(&self) -> UnixTimestamp {
        self.authenticated_at
    }

    pub fn issue_session(
        self,
        state: &mut AccountState,
        expires_at: UnixTimestamp,
    ) -> Result<SessionId, AccountStateError> {
        state.start_session(&self.user_id, self.authenticated_at, expires_at)
    }
}

impl fmt::Display for PasswordAuthenticationServiceInitializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "password authentication service could not be initialized"
        )
    }
}

impl Error for PasswordAuthenticationServiceInitializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Password(error) => Some(error),
            Self::Engine(error) => Some(error),
        }
    }
}

impl<RepositoryError> fmt::Display for PasswordEnrollmentError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountNotFound => write!(f, "account was not found"),
            Self::AccountDisabled => write!(f, "disabled account cannot receive a password"),
            Self::Policy(error) => write!(f, "password does not satisfy policy: {error}"),
            Self::Engine(_) => write!(f, "password could not be protected"),
            Self::Repository(_) => write!(f, "password credential could not be persisted"),
        }
    }
}

impl<RepositoryError> Error for PasswordEnrollmentError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Policy(error) => Some(error),
            Self::Engine(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::AccountNotFound | Self::AccountDisabled => None,
        }
    }
}

impl<RepositoryError> fmt::Display for PasswordAuthenticationError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentials => write!(f, "username or password is invalid"),
            Self::Engine(_) => write!(f, "password verification failed internally"),
            Self::Repository(_) => write!(f, "password credential store is unavailable"),
        }
    }
}

impl<RepositoryError> Error for PasswordAuthenticationError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::InvalidCredentials => None,
        }
    }
}
