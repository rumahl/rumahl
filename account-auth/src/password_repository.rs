use std::error::Error;

use rumahl_core::{UnixTimestamp, UserId};

use crate::PasswordHashRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordCredentialRecord {
    user_id: UserId,
    password_hash: PasswordHashRecord,
    changed_at: UnixTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordAttempt {
    MissingCredential,
    Throttled { retry_at: UnixTimestamp },
    Allowed(PasswordCredentialRecord),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordAttemptPolicy {
    maximum_attempts: u32,
    lockout_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordAttemptPolicyError {
    ZeroMaximumAttempts,
    ZeroLockoutDuration,
}

impl PasswordCredentialRecord {
    pub fn new(
        user_id: UserId,
        password_hash: PasswordHashRecord,
        changed_at: UnixTimestamp,
    ) -> Self {
        Self {
            user_id,
            password_hash,
            changed_at,
        }
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn password_hash(&self) -> &PasswordHashRecord {
        &self.password_hash
    }

    pub fn changed_at(&self) -> UnixTimestamp {
        self.changed_at
    }
}

impl PasswordAttemptPolicy {
    pub fn new(
        maximum_attempts: u32,
        lockout_seconds: u64,
    ) -> Result<Self, PasswordAttemptPolicyError> {
        if maximum_attempts == 0 {
            return Err(PasswordAttemptPolicyError::ZeroMaximumAttempts);
        }

        if lockout_seconds == 0 {
            return Err(PasswordAttemptPolicyError::ZeroLockoutDuration);
        }

        Ok(Self {
            maximum_attempts,
            lockout_seconds,
        })
    }

    pub fn maximum_attempts(self) -> u32 {
        self.maximum_attempts
    }

    pub fn lockout_seconds(self) -> u64 {
        self.lockout_seconds
    }
}

impl Default for PasswordAttemptPolicy {
    fn default() -> Self {
        Self {
            maximum_attempts: 5,
            lockout_seconds: 30,
        }
    }
}

impl std::fmt::Display for PasswordAttemptPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroMaximumAttempts => write!(f, "maximum password attempts must be positive"),
            Self::ZeroLockoutDuration => write!(f, "password lockout duration must be positive"),
        }
    }
}

impl Error for PasswordAttemptPolicyError {}

pub trait PasswordCredentialRepository {
    type Error: Error + Send + Sync + 'static;

    fn replace(&self, credential: &PasswordCredentialRecord) -> Result<(), Self::Error>;

    fn begin_attempt(
        &self,
        user_id: &UserId,
        attempted_at: UnixTimestamp,
        policy: PasswordAttemptPolicy,
    ) -> Result<PasswordAttempt, Self::Error>;

    fn record_success(&self, user_id: &UserId) -> Result<(), Self::Error>;

    fn remove(&self, user_id: &UserId) -> Result<bool, Self::Error>;
}
