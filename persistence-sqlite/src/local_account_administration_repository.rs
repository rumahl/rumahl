use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_account_auth::{LocalAccountAdministrationRepository, PasswordCredentialRecord};
use rumahl_core::{AccountStatus, LocalAccount, SessionId, UnixTimestamp};
use rusqlite::{Connection, TransactionBehavior, params};

use crate::identity_schema;

#[derive(Debug)]
pub struct SqliteLocalAccountAdministrationRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteLocalAccountAdministrationRepositoryError {
    Database(rusqlite::Error),
    CredentialAccountMismatch,
    CredentialMissing,
    SessionStateMismatch(SessionId),
    TimestampOutsideSqliteRange { field: &'static str, value: u64 },
    LockPoisoned,
}

impl SqliteLocalAccountAdministrationRepository {
    pub fn open(
        path: impl AsRef<Path>,
    ) -> Result<Self, SqliteLocalAccountAdministrationRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteLocalAccountAdministrationRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(
        connection: Connection,
    ) -> Result<Self, SqliteLocalAccountAdministrationRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        identity_schema::initialize(&connection).map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteLocalAccountAdministrationRepositoryError {
        SqliteLocalAccountAdministrationRepositoryError::Database(error)
    }
}

impl LocalAccountAdministrationRepository for SqliteLocalAccountAdministrationRepository {
    type Error = SqliteLocalAccountAdministrationRepositoryError;

    fn provision_password_account(
        &self,
        account: &LocalAccount,
        credential: &PasswordCredentialRecord,
    ) -> Result<(), Self::Error> {
        if account.user_id() != credential.user_id() {
            return Err(SqliteLocalAccountAdministrationRepositoryError::CredentialAccountMismatch);
        }

        let changed_at = write_timestamp("changed_at", credential.changed_at())?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteLocalAccountAdministrationRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;

        transaction
            .execute(
                "INSERT INTO local_account (user_id, username, display_name, status)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    account.user_id().to_string(),
                    account.username().as_str(),
                    account.display_name(),
                    status_name(account.status()),
                ],
            )
            .map_err(Self::database_error)?;
        transaction
            .execute(
                "INSERT INTO password_credential (
                     user_id, password_hash, changed_at, failed_attempts, blocked_until
                 ) VALUES (?1, ?2, ?3, 0, NULL)",
                params![
                    credential.user_id().to_string(),
                    credential.password_hash().as_str(),
                    changed_at,
                ],
            )
            .map_err(Self::database_error)?;

        transaction.commit().map_err(Self::database_error)
    }

    fn replace_password_and_revoke_sessions(
        &self,
        credential: &PasswordCredentialRecord,
        session_ids: &[SessionId],
        revoked_at: UnixTimestamp,
    ) -> Result<(), Self::Error> {
        let changed_at = write_timestamp("changed_at", credential.changed_at())?;
        let revoked_at = write_timestamp("revoked_at", revoked_at)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteLocalAccountAdministrationRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;

        let updated = transaction
            .execute(
                "UPDATE password_credential
                 SET password_hash = ?2,
                     changed_at = ?3,
                     failed_attempts = 0,
                     blocked_until = NULL
                 WHERE user_id = ?1",
                params![
                    credential.user_id().to_string(),
                    credential.password_hash().as_str(),
                    changed_at,
                ],
            )
            .map_err(Self::database_error)?;

        if updated != 1 {
            return Err(SqliteLocalAccountAdministrationRepositoryError::CredentialMissing);
        }

        for session_id in session_ids {
            let updated = transaction
                .execute(
                    "UPDATE account_session
                     SET revoked_at = ?3
                     WHERE session_id = ?1 AND user_id = ?2 AND revoked_at IS NULL",
                    params![
                        session_id.to_string(),
                        credential.user_id().to_string(),
                        revoked_at,
                    ],
                )
                .map_err(Self::database_error)?;

            if updated != 1 {
                return Err(
                    SqliteLocalAccountAdministrationRepositoryError::SessionStateMismatch(
                        *session_id,
                    ),
                );
            }

            transaction
                .execute(
                    "UPDATE session_credential
                     SET revoked_at = ?2
                     WHERE session_id = ?1 AND revoked_at IS NULL",
                    params![session_id.to_string(), revoked_at],
                )
                .map_err(Self::database_error)?;
        }

        transaction.commit().map_err(Self::database_error)
    }
}

fn status_name(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Locked => "locked",
        AccountStatus::Disabled => "disabled",
    }
}

fn write_timestamp(
    field: &'static str,
    value: UnixTimestamp,
) -> Result<i64, SqliteLocalAccountAdministrationRepositoryError> {
    i64::try_from(value.as_seconds()).map_err(|_| {
        SqliteLocalAccountAdministrationRepositoryError::TimestampOutsideSqliteRange {
            field,
            value: value.as_seconds(),
        }
    })
}

impl fmt::Display for SqliteLocalAccountAdministrationRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => {
                write!(f, "SQLite local account administration failed: {error}")
            }
            Self::CredentialAccountMismatch => {
                write!(f, "password credential belongs to another account")
            }
            Self::CredentialMissing => write!(f, "password credential was not found"),
            Self::SessionStateMismatch(session_id) => {
                write!(
                    f,
                    "stored session '{session_id}' is not active for this account"
                )
            }
            Self::TimestampOutsideSqliteRange { field, value } => {
                write!(
                    f,
                    "{field} timestamp {value} is outside SQLite integer range"
                )
            }
            Self::LockPoisoned => {
                write!(f, "SQLite local account administration lock is poisoned")
            }
        }
    }
}

impl Error for SqliteLocalAccountAdministrationRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::CredentialAccountMismatch
            | Self::CredentialMissing
            | Self::SessionStateMismatch(_)
            | Self::TimestampOutsideSqliteRange { .. }
            | Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_account_auth::{
        AccountProvisioningError, LocalAccountAdministrationService, PasswordAuthenticationError,
        PasswordAuthenticationService, PasswordBlocklist, PasswordChangeError,
        SessionCredentialIssuer, SessionCredentialRepository,
    };
    use rumahl_core::{AccountState, AccountStateRepository};

    use crate::{
        SqliteAccountStateRepository, SqlitePasswordCredentialRepository,
        SqliteSessionCredentialRepository,
    };

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
    fn provisions_account_and_password_in_one_transaction() {
        let path = database_path("account-provisioning");
        let mut state = AccountState::new();
        let administration = LocalAccountAdministrationService::new(
            SqliteLocalAccountAdministrationRepository::open(&path).unwrap(),
            TestBlocklist,
        );

        let user_id = administration
            .provision_password_account(
                &mut state,
                "kai",
                "Kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();
        let recovered = SqliteAccountStateRepository::open(&path)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(
            recovered.accounts().get(&user_id).unwrap().display_name(),
            "Kai"
        );
        assert!(
            PasswordAuthenticationService::new(
                SqlitePasswordCredentialRepository::open(&path).unwrap(),
                TestBlocklist,
            )
            .unwrap()
            .authenticate(
                &recovered,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .is_ok()
        );

        drop(administration);
        remove_database(&path);
    }

    #[test]
    fn failed_password_insert_leaves_no_partial_account() {
        let repository = SqliteLocalAccountAdministrationRepository::open_in_memory().unwrap();
        repository
            .connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_password_insert
                 BEFORE INSERT ON password_credential
                 BEGIN
                     SELECT RAISE(ABORT, 'test password insert failure');
                 END;",
            )
            .unwrap();
        let administration = LocalAccountAdministrationService::new(repository, TestBlocklist);
        let mut state = AccountState::new();

        let error = administration
            .provision_password_account(
                &mut state,
                "kai",
                "Kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap_err();

        assert!(matches!(error, AccountProvisioningError::Repository(_)));
        assert!(state.accounts().is_empty());
        let connection = administration.repository().connection.lock().unwrap();
        let account_count = connection
            .query_row("SELECT COUNT(*) FROM local_account", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let credential_count = connection
            .query_row("SELECT COUNT(*) FROM password_credential", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(account_count, 0);
        assert_eq!(credential_count, 0);
    }

    #[test]
    fn password_change_revokes_all_os_sessions_and_transport_credentials() {
        let path = database_path("password-change");
        let administration = LocalAccountAdministrationService::new(
            SqliteLocalAccountAdministrationRepository::open(&path).unwrap(),
            TestBlocklist,
        );
        let mut state = AccountState::new();
        let user_id = administration
            .provision_password_account(
                &mut state,
                "kai",
                "Kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();
        let other_user_id = administration
            .provision_password_account(
                &mut state,
                "lea",
                "Lea",
                "another correct battery passphrase".to_owned(),
                UnixTimestamp::from_seconds(110),
            )
            .unwrap();
        let first_session = state
            .start_session(
                &user_id,
                UnixTimestamp::from_seconds(150),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        let second_session = state
            .start_session(
                &user_id,
                UnixTimestamp::from_seconds(160),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        let other_session = state
            .start_session(
                &other_user_id,
                UnixTimestamp::from_seconds(165),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        SqliteAccountStateRepository::open(&path)
            .unwrap()
            .store(&state)
            .unwrap();

        let first_token =
            SessionCredentialIssuer::new(SqliteSessionCredentialRepository::open(&path).unwrap())
                .issue(first_session, UnixTimestamp::from_seconds(170))
                .unwrap();
        let second_token =
            SessionCredentialIssuer::new(SqliteSessionCredentialRepository::open(&path).unwrap())
                .issue(second_session, UnixTimestamp::from_seconds(171))
                .unwrap();
        let other_token =
            SessionCredentialIssuer::new(SqliteSessionCredentialRepository::open(&path).unwrap())
                .issue(other_session, UnixTimestamp::from_seconds(172))
                .unwrap();
        let authentication = PasswordAuthenticationService::new(
            SqlitePasswordCredentialRepository::open(&path).unwrap(),
            TestBlocklist,
        )
        .unwrap();
        let verified = authentication
            .authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        assert_eq!(
            administration
                .change_password(
                    &mut state,
                    verified,
                    "a newer horse battery passphrase".to_owned(),
                    UnixTimestamp::from_seconds(210),
                )
                .unwrap(),
            2
        );
        assert!(
            state
                .sessions()
                .sessions_for_user(&user_id)
                .all(|session| session.revoked_at() == Some(UnixTimestamp::from_seconds(210)))
        );
        assert!(
            state
                .sessions()
                .get(&other_session)
                .unwrap()
                .revoked_at()
                .is_none()
        );

        let recovered = SqliteAccountStateRepository::open(&path)
            .unwrap()
            .load()
            .unwrap();
        assert!(
            recovered
                .sessions()
                .sessions_for_user(&user_id)
                .all(|session| session.revoked_at() == Some(UnixTimestamp::from_seconds(210)))
        );
        let session_credentials = SqliteSessionCredentialRepository::open(&path).unwrap();
        for token in [&first_token, &second_token] {
            assert_eq!(session_credentials.resolve(&token.digest()).unwrap(), None);
        }
        assert_eq!(
            session_credentials.resolve(&other_token.digest()).unwrap(),
            Some(other_session)
        );
        assert!(matches!(
            authentication.authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(220),
            ),
            Err(PasswordAuthenticationError::InvalidCredentials)
        ));
        assert!(
            authentication
                .authenticate(
                    &state,
                    "kai",
                    "a newer horse battery passphrase".to_owned(),
                    UnixTimestamp::from_seconds(221),
                )
                .is_ok()
        );

        drop(session_credentials);
        drop(authentication);
        drop(administration);
        remove_database(&path);
    }

    #[test]
    fn failed_session_revocation_rolls_back_password_and_live_state() {
        let path = database_path("password-change-rollback");
        let administration = LocalAccountAdministrationService::new(
            SqliteLocalAccountAdministrationRepository::open(&path).unwrap(),
            TestBlocklist,
        );
        let mut state = AccountState::new();
        let user_id = administration
            .provision_password_account(
                &mut state,
                "kai",
                "Kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();
        let session_id = state
            .start_session(
                &user_id,
                UnixTimestamp::from_seconds(150),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        SqliteAccountStateRepository::open(&path)
            .unwrap()
            .store(&state)
            .unwrap();
        let authentication = PasswordAuthenticationService::new(
            SqlitePasswordCredentialRepository::open(&path).unwrap(),
            TestBlocklist,
        )
        .unwrap();
        let verified = authentication
            .authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();
        administration
            .repository()
            .connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_session_revocation
                 BEFORE UPDATE OF revoked_at ON account_session
                 BEGIN
                     SELECT RAISE(ABORT, 'test session revocation failure');
                 END;",
            )
            .unwrap();

        assert!(
            administration
                .change_password(
                    &mut state,
                    verified,
                    "a newer horse battery passphrase".to_owned(),
                    UnixTimestamp::from_seconds(210),
                )
                .is_err()
        );
        assert!(
            state
                .sessions()
                .get(&session_id)
                .unwrap()
                .revoked_at()
                .is_none()
        );
        assert!(
            authentication
                .authenticate(
                    &state,
                    "kai",
                    "correct horse battery staple".to_owned(),
                    UnixTimestamp::from_seconds(220),
                )
                .is_ok()
        );
        assert!(matches!(
            authentication.authenticate(
                &state,
                "kai",
                "a newer horse battery passphrase".to_owned(),
                UnixTimestamp::from_seconds(221),
            ),
            Err(PasswordAuthenticationError::InvalidCredentials)
        ));

        drop(authentication);
        drop(administration);
        remove_database(&path);
    }

    #[test]
    fn password_change_rejects_expired_authentication_proof() {
        let path = database_path("password-change-expired-proof");
        let administration = LocalAccountAdministrationService::new(
            SqliteLocalAccountAdministrationRepository::open(&path).unwrap(),
            TestBlocklist,
        );
        let mut state = AccountState::new();
        administration
            .provision_password_account(
                &mut state,
                "kai",
                "Kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();
        let authentication = PasswordAuthenticationService::new(
            SqlitePasswordCredentialRepository::open(&path).unwrap(),
            TestBlocklist,
        )
        .unwrap();
        let verified = authentication
            .authenticate(
                &state,
                "kai",
                "correct horse battery staple".to_owned(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        assert!(matches!(
            administration.change_password(
                &mut state,
                verified,
                "a newer horse battery passphrase".to_owned(),
                UnixTimestamp::from_seconds(501),
            ),
            Err(PasswordChangeError::AuthenticationExpired)
        ));
        assert!(
            authentication
                .authenticate(
                    &state,
                    "kai",
                    "correct horse battery staple".to_owned(),
                    UnixTimestamp::from_seconds(502),
                )
                .is_ok()
        );

        drop(authentication);
        drop(administration);
        remove_database(&path);
    }
}
