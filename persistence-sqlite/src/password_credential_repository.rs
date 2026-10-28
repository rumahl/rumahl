use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_account_auth::{
    PasswordAttempt, PasswordAttemptPolicy, PasswordCredentialRecord, PasswordCredentialRepository,
    PasswordHashRecord,
};
use rumahl_core::{UnixTimestamp, UserId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::identity_schema;

#[derive(Debug)]
pub struct SqlitePasswordCredentialRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqlitePasswordCredentialRepositoryError {
    Database(rusqlite::Error),
    InvalidPasswordHash,
    InvalidTimestamp { field: &'static str, value: i64 },
    TimestampOutsideSqliteRange { field: &'static str, value: u64 },
    InvalidFailedAttemptCount(i64),
    AttemptCounterOverflow,
    LockoutTimestampOverflow,
    CredentialMissing,
    LockPoisoned,
}

impl SqlitePasswordCredentialRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePasswordCredentialRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqlitePasswordCredentialRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(
        connection: Connection,
    ) -> Result<Self, SqlitePasswordCredentialRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        identity_schema::initialize(&connection).map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqlitePasswordCredentialRepositoryError {
        SqlitePasswordCredentialRepositoryError::Database(error)
    }
}

impl PasswordCredentialRepository for SqlitePasswordCredentialRepository {
    type Error = SqlitePasswordCredentialRepositoryError;

    fn replace(&self, credential: &PasswordCredentialRecord) -> Result<(), Self::Error> {
        let changed_at = write_timestamp("changed_at", credential.changed_at())?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePasswordCredentialRepositoryError::LockPoisoned)?;

        connection
            .execute(
                "INSERT INTO password_credential (
                     user_id, password_hash, changed_at, failed_attempts, blocked_until
                 ) VALUES (?1, ?2, ?3, 0, NULL)
                 ON CONFLICT(user_id) DO UPDATE SET
                     password_hash = excluded.password_hash,
                     changed_at = excluded.changed_at,
                     failed_attempts = 0,
                     blocked_until = NULL",
                params![
                    credential.user_id().to_string(),
                    credential.password_hash().as_str(),
                    changed_at,
                ],
            )
            .map_err(Self::database_error)?;

        Ok(())
    }

    fn begin_attempt(
        &self,
        user_id: &UserId,
        attempted_at: UnixTimestamp,
        policy: PasswordAttemptPolicy,
    ) -> Result<PasswordAttempt, Self::Error> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePasswordCredentialRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;
        let stored = transaction
            .query_row(
                "SELECT password_hash, changed_at, failed_attempts, blocked_until
                 FROM password_credential
                 WHERE user_id = ?1",
                [user_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(Self::database_error)?;

        let Some((password_hash, changed_at, failed_attempts, blocked_until)) = stored else {
            transaction.commit().map_err(Self::database_error)?;
            return Ok(PasswordAttempt::MissingCredential);
        };

        let credential = PasswordCredentialRecord::new(
            *user_id,
            PasswordHashRecord::parse(password_hash)
                .map_err(|_| SqlitePasswordCredentialRepositoryError::InvalidPasswordHash)?,
            read_timestamp("changed_at", changed_at)?,
        );
        let failed_attempts = u32::try_from(failed_attempts).map_err(|_| {
            SqlitePasswordCredentialRepositoryError::InvalidFailedAttemptCount(failed_attempts)
        })?;
        let blocked_until = blocked_until
            .map(|value| read_timestamp("blocked_until", value))
            .transpose()?;

        if let Some(retry_at) = blocked_until.filter(|blocked_until| *blocked_until > attempted_at)
        {
            transaction.commit().map_err(Self::database_error)?;
            return Ok(PasswordAttempt::Throttled { retry_at });
        }

        let previous_attempts = if blocked_until.is_some() {
            0
        } else {
            failed_attempts
        };
        let attempts = previous_attempts
            .checked_add(1)
            .ok_or(SqlitePasswordCredentialRepositoryError::AttemptCounterOverflow)?;
        let next_blocked_until = if attempts >= policy.maximum_attempts() {
            Some(UnixTimestamp::from_seconds(
                attempted_at
                    .as_seconds()
                    .checked_add(policy.lockout_seconds())
                    .ok_or(SqlitePasswordCredentialRepositoryError::LockoutTimestampOverflow)?,
            ))
        } else {
            None
        };

        transaction
            .execute(
                "UPDATE password_credential
                 SET failed_attempts = ?2, blocked_until = ?3
                 WHERE user_id = ?1",
                params![
                    user_id.to_string(),
                    i64::from(attempts),
                    next_blocked_until
                        .map(|timestamp| write_timestamp("blocked_until", timestamp))
                        .transpose()?,
                ],
            )
            .map_err(Self::database_error)?;
        transaction.commit().map_err(Self::database_error)?;

        Ok(PasswordAttempt::Allowed(credential))
    }

    fn record_success(&self, user_id: &UserId) -> Result<(), Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePasswordCredentialRepositoryError::LockPoisoned)?;
        let updated = connection
            .execute(
                "UPDATE password_credential
                 SET failed_attempts = 0, blocked_until = NULL
                 WHERE user_id = ?1",
                [user_id.to_string()],
            )
            .map_err(Self::database_error)?;

        if updated == 0 {
            return Err(SqlitePasswordCredentialRepositoryError::CredentialMissing);
        }

        Ok(())
    }

    fn remove(&self, user_id: &UserId) -> Result<bool, Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePasswordCredentialRepositoryError::LockPoisoned)?;

        connection
            .execute(
                "DELETE FROM password_credential WHERE user_id = ?1",
                [user_id.to_string()],
            )
            .map(|removed| removed > 0)
            .map_err(Self::database_error)
    }
}

fn read_timestamp(
    field: &'static str,
    value: i64,
) -> Result<UnixTimestamp, SqlitePasswordCredentialRepositoryError> {
    let value = u64::try_from(value)
        .map_err(|_| SqlitePasswordCredentialRepositoryError::InvalidTimestamp { field, value })?;

    Ok(UnixTimestamp::from_seconds(value))
}

fn write_timestamp(
    field: &'static str,
    value: UnixTimestamp,
) -> Result<i64, SqlitePasswordCredentialRepositoryError> {
    i64::try_from(value.as_seconds()).map_err(|_| {
        SqlitePasswordCredentialRepositoryError::TimestampOutsideSqliteRange {
            field,
            value: value.as_seconds(),
        }
    })
}

impl fmt::Display for SqlitePasswordCredentialRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => {
                write!(f, "SQLite password credential repository failed: {error}")
            }
            Self::InvalidPasswordHash => write!(f, "stored password verifier is invalid"),
            Self::InvalidTimestamp { field, value } => {
                write!(f, "stored {field} timestamp {value} is invalid")
            }
            Self::TimestampOutsideSqliteRange { field, value } => {
                write!(
                    f,
                    "{field} timestamp {value} is outside SQLite integer range"
                )
            }
            Self::InvalidFailedAttemptCount(value) => {
                write!(f, "stored failed-attempt count {value} is invalid")
            }
            Self::AttemptCounterOverflow => write!(f, "password attempt counter overflowed"),
            Self::LockoutTimestampOverflow => write!(f, "password lockout timestamp overflowed"),
            Self::CredentialMissing => write!(f, "password credential was not found"),
            Self::LockPoisoned => {
                write!(f, "SQLite password credential repository lock is poisoned")
            }
        }
    }
}

impl Error for SqlitePasswordCredentialRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidPasswordHash
            | Self::InvalidTimestamp { .. }
            | Self::TimestampOutsideSqliteRange { .. }
            | Self::InvalidFailedAttemptCount(_)
            | Self::AttemptCounterOverflow
            | Self::LockoutTimestampOverflow
            | Self::CredentialMissing
            | Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_account_auth::{
        Argon2idPasswordEngine, Password, PasswordAuthenticationError,
        PasswordAuthenticationService, PasswordBlocklist, PasswordEnrollmentError,
    };
    use rumahl_core::{AccountState, AccountStatus};

    use super::*;

    struct TestBlocklist;

    impl PasswordBlocklist for TestBlocklist {
        fn contains(&self, normalized_password: &str) -> bool {
            normalized_password == "password password password"
        }
    }

    fn database_path(test_name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("rumahl-{test_name}-{nonce}.sqlite3"))
    }

    fn remove_database(path: &Path) {
        for suffix in ["", "-shm", "-wal"] {
            let candidate = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
            let _ = fs::remove_file(candidate);
        }
    }

    #[test]
    fn password_credential_and_throttle_survive_reopen() {
        let path = database_path("password-credential");
        let user_id = UserId::new();
        let password = Password::prepare("correct horse battery staple".to_owned()).unwrap();
        let hash = Argon2idPasswordEngine::new().hash(&password).unwrap();
        let record = PasswordCredentialRecord::new(user_id, hash, UnixTimestamp::from_seconds(100));

        {
            let repository = SqlitePasswordCredentialRepository::open(&path).unwrap();
            repository.replace(&record).unwrap();

            for _ in 0..5 {
                assert!(matches!(
                    repository
                        .begin_attempt(
                            &user_id,
                            UnixTimestamp::from_seconds(200),
                            PasswordAttemptPolicy::default(),
                        )
                        .unwrap(),
                    PasswordAttempt::Allowed(_)
                ));
            }
        }

        let repository = SqlitePasswordCredentialRepository::open(&path).unwrap();

        assert_eq!(
            repository
                .begin_attempt(
                    &user_id,
                    UnixTimestamp::from_seconds(201),
                    PasswordAttemptPolicy::default(),
                )
                .unwrap(),
            PasswordAttempt::Throttled {
                retry_at: UnixTimestamp::from_seconds(230)
            }
        );
        assert!(matches!(
            repository
                .begin_attempt(
                    &user_id,
                    UnixTimestamp::from_seconds(230),
                    PasswordAttemptPolicy::default(),
                )
                .unwrap(),
            PasswordAttempt::Allowed(_)
        ));

        drop(repository);
        remove_database(&path);
    }

    #[test]
    fn authenticates_local_account_and_issues_os_session() {
        let repository = SqlitePasswordCredentialRepository::open_in_memory().unwrap();
        let service = PasswordAuthenticationService::new(repository, TestBlocklist).unwrap();
        let mut state = AccountState::new();
        let user_id = state.create_account("kai", "Kai").unwrap();

        service
            .set_password(
                &state,
                &user_id,
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();

        let verified = service
            .authenticate(
                &state,
                "KAI",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();
        let session_id = verified
            .issue_session(&mut state, UnixTimestamp::from_seconds(1_000))
            .unwrap();

        assert_eq!(
            state.sessions().get(&session_id).unwrap().user_id(),
            &user_id
        );
    }

    #[test]
    fn wrong_unknown_and_locked_accounts_share_generic_failure() {
        let repository = SqlitePasswordCredentialRepository::open_in_memory().unwrap();
        let service = PasswordAuthenticationService::new(repository, TestBlocklist).unwrap();
        let mut state = AccountState::new();
        let user_id = state.create_account("kai", "Kai").unwrap();
        service
            .set_password(
                &state,
                &user_id,
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();

        let wrong = service
            .authenticate(
                &state,
                "kai",
                "wrong password but long enough".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap_err();
        let unknown = service
            .authenticate(
                &state,
                "unknown",
                "wrong password but long enough".to_owned(),
                UnixTimestamp::from_seconds(201),
            )
            .unwrap_err();
        state
            .lock_account(&user_id, UnixTimestamp::from_seconds(202))
            .unwrap();
        let locked = service
            .authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(203),
            )
            .unwrap_err();

        assert!(matches!(
            wrong,
            PasswordAuthenticationError::InvalidCredentials
        ));
        assert!(matches!(
            unknown,
            PasswordAuthenticationError::InvalidCredentials
        ));
        assert!(matches!(
            locked,
            PasswordAuthenticationError::InvalidCredentials
        ));
        assert_eq!(wrong.to_string(), unknown.to_string());
        assert_eq!(unknown.to_string(), locked.to_string());
        assert_eq!(
            state.accounts().get(&user_id).unwrap().status(),
            AccountStatus::Locked
        );
    }

    #[test]
    fn blocklisted_password_is_not_persisted() {
        let repository = SqlitePasswordCredentialRepository::open_in_memory().unwrap();
        let service = PasswordAuthenticationService::new(repository, TestBlocklist).unwrap();
        let mut state = AccountState::new();
        let user_id = state.create_account("kai", "Kai").unwrap();

        let error = service
            .set_password(
                &state,
                &user_id,
                "password password password".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            PasswordEnrollmentError::Policy(rumahl_account_auth::PasswordPolicyError::Blocked)
        ));
        assert!(matches!(
            service
                .repository()
                .begin_attempt(
                    &user_id,
                    UnixTimestamp::from_seconds(200),
                    PasswordAttemptPolicy::default(),
                )
                .unwrap(),
            PasswordAttempt::MissingCredential
        ));
    }

    #[test]
    fn authentication_enforces_persisted_attempt_lockout() {
        let repository = SqlitePasswordCredentialRepository::open_in_memory().unwrap();
        let service = PasswordAuthenticationService::with_policies(
            repository,
            TestBlocklist,
            rumahl_account_auth::PasswordPolicy::default(),
            PasswordAttemptPolicy::new(2, 10).unwrap(),
        )
        .unwrap();
        let mut state = AccountState::new();
        let user_id = state.create_account("kai", "Kai").unwrap();
        service
            .set_password(
                &state,
                &user_id,
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();

        for attempted_at in [200, 201] {
            assert!(matches!(
                service.authenticate(
                    &state,
                    "kai",
                    "wrong password but long enough".to_owned(),
                    UnixTimestamp::from_seconds(attempted_at),
                ),
                Err(PasswordAuthenticationError::InvalidCredentials)
            ));
        }

        assert!(matches!(
            service.authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(202),
            ),
            Err(PasswordAuthenticationError::InvalidCredentials)
        ));
        assert!(
            service
                .authenticate(
                    &state,
                    "kai",
                    "correct horse battery staple".to_owned(),
                    UnixTimestamp::from_seconds(211),
                )
                .is_ok()
        );
    }
}
