use std::error::Error;
use std::fmt;

use rumahl_core::{
    AccountState, AccountStateError, AccountStatus, SessionId, UnixTimestamp, UserId,
};

use crate::{
    Argon2idPasswordEngine, LocalAccountAdministrationRepository, Password, PasswordBlocklist,
    PasswordCredentialRecord, PasswordEngineError, PasswordPolicy, PasswordPolicyError,
    VerifiedLocalAccount,
};

#[derive(Debug)]
pub struct LocalAccountAdministrationService<R, B> {
    repository: R,
    blocklist: B,
    password_policy: PasswordPolicy,
    password_change_policy: PasswordChangePolicy,
    engine: Argon2idPasswordEngine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordChangePolicy {
    maximum_authentication_age_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordChangePolicyError {
    ZeroMaximumAuthenticationAge,
}

#[derive(Debug)]
pub enum AccountProvisioningError<RepositoryError> {
    Account(AccountStateError),
    Policy(PasswordPolicyError),
    Engine(PasswordEngineError),
    Repository(RepositoryError),
}

#[derive(Debug)]
pub enum PasswordChangeError<RepositoryError> {
    AccountNotFound,
    AccountUnavailable,
    AuthenticationFromFuture,
    AuthenticationExpired,
    Policy(PasswordPolicyError),
    Engine(PasswordEngineError),
    Account(AccountStateError),
    Repository(RepositoryError),
}

impl<R, B> LocalAccountAdministrationService<R, B>
where
    R: LocalAccountAdministrationRepository,
    B: PasswordBlocklist,
{
    pub fn new(repository: R, blocklist: B) -> Self {
        Self::with_password_policy(repository, blocklist, PasswordPolicy::default())
    }

    pub fn with_password_policy(
        repository: R,
        blocklist: B,
        password_policy: PasswordPolicy,
    ) -> Self {
        Self::with_policies(
            repository,
            blocklist,
            password_policy,
            PasswordChangePolicy::default(),
        )
    }

    pub fn with_policies(
        repository: R,
        blocklist: B,
        password_policy: PasswordPolicy,
        password_change_policy: PasswordChangePolicy,
    ) -> Self {
        Self {
            repository,
            blocklist,
            password_policy,
            password_change_policy,
            engine: Argon2idPasswordEngine::new(),
        }
    }

    pub fn provision_password_account(
        &self,
        state: &mut AccountState,
        username: &str,
        display_name: &str,
        candidate: String,
        created_at: UnixTimestamp,
    ) -> Result<UserId, AccountProvisioningError<R::Error>> {
        let mut staged = state.clone();
        let user_id = staged
            .create_account(username, display_name)
            .map_err(AccountProvisioningError::Account)?;
        let credential = self
            .prepare_credential(user_id, candidate, created_at)
            .map_err(map_provisioning_credential_error)?;
        let account = staged
            .accounts()
            .get(&user_id)
            .expect("new account exists in staged state");

        self.repository
            .provision_password_account(account, &credential)
            .map_err(AccountProvisioningError::Repository)?;

        *state = staged;

        Ok(user_id)
    }

    pub fn change_password(
        &self,
        state: &mut AccountState,
        verified: VerifiedLocalAccount,
        candidate: String,
        changed_at: UnixTimestamp,
    ) -> Result<usize, PasswordChangeError<R::Error>> {
        if verified.authenticated_at() > changed_at {
            return Err(PasswordChangeError::AuthenticationFromFuture);
        }

        if changed_at.as_seconds() - verified.authenticated_at().as_seconds()
            > self
                .password_change_policy
                .maximum_authentication_age_seconds()
        {
            return Err(PasswordChangeError::AuthenticationExpired);
        }

        let user_id = *verified.user_id();
        let account = state
            .accounts()
            .get(&user_id)
            .ok_or(PasswordChangeError::AccountNotFound)?;

        if account.status() != AccountStatus::Active {
            return Err(PasswordChangeError::AccountUnavailable);
        }

        let credential = self
            .prepare_credential(user_id, candidate, changed_at)
            .map_err(map_password_change_credential_error)?;
        let session_ids = state
            .sessions()
            .sessions_for_user(&user_id)
            .filter(|session| session.revoked_at().is_none())
            .map(|session| *session.session_id())
            .collect::<Vec<SessionId>>();
        let mut staged = state.clone();
        let revoked = staged
            .revoke_sessions_for_user(&user_id, changed_at)
            .map_err(PasswordChangeError::Account)?;

        self.repository
            .replace_password_and_revoke_sessions(&credential, &session_ids, changed_at)
            .map_err(PasswordChangeError::Repository)?;

        *state = staged;

        Ok(revoked)
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    fn prepare_credential(
        &self,
        user_id: UserId,
        candidate: String,
        changed_at: UnixTimestamp,
    ) -> Result<PasswordCredentialRecord, PreparedCredentialError> {
        let password = Password::prepare(candidate).map_err(PreparedCredentialError::Policy)?;
        self.password_policy
            .validate(&password, &self.blocklist)
            .map_err(PreparedCredentialError::Policy)?;
        let password_hash = self
            .engine
            .hash(&password)
            .map_err(PreparedCredentialError::Engine)?;

        Ok(PasswordCredentialRecord::new(
            user_id,
            password_hash,
            changed_at,
        ))
    }
}

impl PasswordChangePolicy {
    pub fn new(maximum_authentication_age_seconds: u64) -> Result<Self, PasswordChangePolicyError> {
        if maximum_authentication_age_seconds == 0 {
            return Err(PasswordChangePolicyError::ZeroMaximumAuthenticationAge);
        }

        Ok(Self {
            maximum_authentication_age_seconds,
        })
    }

    pub fn maximum_authentication_age_seconds(self) -> u64 {
        self.maximum_authentication_age_seconds
    }
}

impl Default for PasswordChangePolicy {
    fn default() -> Self {
        Self {
            maximum_authentication_age_seconds: 300,
        }
    }
}

enum PreparedCredentialError {
    Policy(PasswordPolicyError),
    Engine(PasswordEngineError),
}

fn map_provisioning_credential_error<RepositoryError>(
    error: PreparedCredentialError,
) -> AccountProvisioningError<RepositoryError> {
    match error {
        PreparedCredentialError::Policy(error) => AccountProvisioningError::Policy(error),
        PreparedCredentialError::Engine(error) => AccountProvisioningError::Engine(error),
    }
}

fn map_password_change_credential_error<RepositoryError>(
    error: PreparedCredentialError,
) -> PasswordChangeError<RepositoryError> {
    match error {
        PreparedCredentialError::Policy(error) => PasswordChangeError::Policy(error),
        PreparedCredentialError::Engine(error) => PasswordChangeError::Engine(error),
    }
}

impl<RepositoryError> fmt::Display for AccountProvisioningError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account(error) => write!(f, "account could not be created: {error}"),
            Self::Policy(error) => write!(f, "password does not satisfy policy: {error}"),
            Self::Engine(_) => write!(f, "password could not be protected"),
            Self::Repository(_) => write!(f, "account could not be provisioned atomically"),
        }
    }
}

impl<RepositoryError> Error for AccountProvisioningError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Account(error) => Some(error),
            Self::Policy(error) => Some(error),
            Self::Engine(error) => Some(error),
            Self::Repository(error) => Some(error),
        }
    }
}

impl<RepositoryError> fmt::Display for PasswordChangeError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountNotFound => write!(f, "account was not found"),
            Self::AccountUnavailable => write!(f, "account is unavailable"),
            Self::AuthenticationFromFuture => {
                write!(f, "authentication proof is newer than the password change")
            }
            Self::AuthenticationExpired => {
                write!(f, "fresh authentication is required to change the password")
            }
            Self::Policy(error) => write!(f, "password does not satisfy policy: {error}"),
            Self::Engine(_) => write!(f, "password could not be protected"),
            Self::Account(error) => write!(f, "account sessions could not be revoked: {error}"),
            Self::Repository(_) => write!(f, "password could not be changed atomically"),
        }
    }
}

impl<RepositoryError> Error for PasswordChangeError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Policy(error) => Some(error),
            Self::Engine(error) => Some(error),
            Self::Account(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::AccountNotFound
            | Self::AccountUnavailable
            | Self::AuthenticationFromFuture
            | Self::AuthenticationExpired => None,
        }
    }
}

impl fmt::Display for PasswordChangePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaximumAuthenticationAge => {
                write!(f, "maximum authentication age must be positive")
            }
        }
    }
}

impl Error for PasswordChangePolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_change_policy_requires_a_positive_reauthentication_window() {
        assert_eq!(
            PasswordChangePolicy::new(0).unwrap_err(),
            PasswordChangePolicyError::ZeroMaximumAuthenticationAge
        );
        assert_eq!(
            PasswordChangePolicy::new(120)
                .unwrap()
                .maximum_authentication_age_seconds(),
            120
        );
    }
}
