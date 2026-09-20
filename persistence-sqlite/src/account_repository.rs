use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{
    AccountSession, AccountState, AccountStateError, AccountStateRepository, AccountStatus,
    AccountUsername, LocalAccount, SessionId, UnixTimestamp, UserId,
};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

#[derive(Debug)]
pub struct SqliteAccountStateRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteAccountStateRepositoryError {
    Database(rusqlite::Error),
    InvalidAccount(String),
    InvalidSession(String),
    InvalidState(AccountStateError),
    InvalidStatus(String),
    InvalidTimestamp { field: &'static str, value: i64 },
    TimestampOutsideSqliteRange { field: &'static str, value: u64 },
    LockPoisoned,
}

impl SqliteAccountStateRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteAccountStateRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteAccountStateRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteAccountStateRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS local_account (
                     user_id TEXT PRIMARY KEY,
                     username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                     display_name TEXT NOT NULL,
                     status TEXT NOT NULL CHECK (status IN ('active', 'locked', 'disabled'))
                 );

                 CREATE TABLE IF NOT EXISTS account_session (
                     session_id TEXT PRIMARY KEY,
                     user_id TEXT NOT NULL REFERENCES local_account(user_id) ON DELETE CASCADE,
                     authenticated_at INTEGER NOT NULL CHECK (authenticated_at >= 0),
                     last_seen_at INTEGER NOT NULL CHECK (last_seen_at >= authenticated_at),
                     reauthenticated_at INTEGER,
                     expires_at INTEGER NOT NULL CHECK (expires_at > authenticated_at),
                     revoked_at INTEGER,
                     CHECK (last_seen_at < expires_at),
                     CHECK (
                         reauthenticated_at IS NULL OR
                         (reauthenticated_at >= authenticated_at AND reauthenticated_at <= last_seen_at)
                     ),
                     CHECK (revoked_at IS NULL OR revoked_at >= authenticated_at)
                 );

                 CREATE INDEX IF NOT EXISTS account_session_user_id
                 ON account_session(user_id);",
            )
            .map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteAccountStateRepositoryError {
        SqliteAccountStateRepositoryError::Database(error)
    }

    fn load_accounts(
        connection: &Connection,
    ) -> Result<Vec<LocalAccount>, SqliteAccountStateRepositoryError> {
        let mut statement = connection
            .prepare(
                "SELECT user_id, username, display_name, status
                 FROM local_account
                 ORDER BY username",
            )
            .map_err(Self::database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(Self::database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Self::database_error)?;

        rows.into_iter()
            .map(|(user_id, username, display_name, status)| {
                let user_id = UserId::parse(&user_id).map_err(|error| {
                    SqliteAccountStateRepositoryError::InvalidAccount(error.to_string())
                })?;
                let username = AccountUsername::parse(&username).map_err(|error| {
                    SqliteAccountStateRepositoryError::InvalidAccount(error.to_string())
                })?;
                let status = parse_status(&status)?;

                LocalAccount::restore(user_id, username, &display_name, status).map_err(|error| {
                    SqliteAccountStateRepositoryError::InvalidAccount(error.to_string())
                })
            })
            .collect()
    }

    fn load_sessions(
        connection: &Connection,
    ) -> Result<Vec<AccountSession>, SqliteAccountStateRepositoryError> {
        type SessionRow = (String, String, i64, i64, Option<i64>, i64, Option<i64>);

        let mut statement = connection
            .prepare(
                "SELECT session_id, user_id, authenticated_at, last_seen_at,
                        reauthenticated_at, expires_at, revoked_at
                 FROM account_session
                 ORDER BY session_id",
            )
            .map_err(Self::database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            })
            .map_err(Self::database_error)?
            .collect::<Result<Vec<SessionRow>, _>>()
            .map_err(Self::database_error)?;

        rows.into_iter()
            .map(
                |(
                    session_id,
                    user_id,
                    authenticated_at,
                    last_seen_at,
                    reauthenticated_at,
                    expires_at,
                    revoked_at,
                )| {
                    let session_id = SessionId::parse(&session_id).map_err(|error| {
                        SqliteAccountStateRepositoryError::InvalidSession(error.to_string())
                    })?;
                    let user_id = UserId::parse(&user_id).map_err(|error| {
                        SqliteAccountStateRepositoryError::InvalidSession(error.to_string())
                    })?;

                    AccountSession::restore(
                        session_id,
                        user_id,
                        read_timestamp("authenticated_at", authenticated_at)?,
                        read_timestamp("last_seen_at", last_seen_at)?,
                        read_optional_timestamp("reauthenticated_at", reauthenticated_at)?,
                        read_timestamp("expires_at", expires_at)?,
                        read_optional_timestamp("revoked_at", revoked_at)?,
                    )
                    .map_err(|error| {
                        SqliteAccountStateRepositoryError::InvalidSession(error.to_string())
                    })
                },
            )
            .collect()
    }

    fn store_account(
        transaction: &Transaction<'_>,
        account: &LocalAccount,
    ) -> Result<(), SqliteAccountStateRepositoryError> {
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

        Ok(())
    }

    fn store_session(
        transaction: &Transaction<'_>,
        session: &AccountSession,
    ) -> Result<(), SqliteAccountStateRepositoryError> {
        transaction
            .execute(
                "INSERT INTO account_session (
                     session_id, user_id, authenticated_at, last_seen_at,
                     reauthenticated_at, expires_at, revoked_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    session.session_id().to_string(),
                    session.user_id().to_string(),
                    write_timestamp("authenticated_at", session.authenticated_at())?,
                    write_timestamp("last_seen_at", session.last_seen_at())?,
                    write_optional_timestamp("reauthenticated_at", session.reauthenticated_at())?,
                    write_timestamp("expires_at", session.expires_at())?,
                    write_optional_timestamp("revoked_at", session.revoked_at())?,
                ],
            )
            .map_err(Self::database_error)?;

        Ok(())
    }
}

impl AccountStateRepository for SqliteAccountStateRepository {
    type Error = SqliteAccountStateRepositoryError;

    fn load(&self) -> Result<AccountState, Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteAccountStateRepositoryError::LockPoisoned)?;
        let accounts = Self::load_accounts(&connection)?;
        let sessions = Self::load_sessions(&connection)?;

        AccountState::restore(accounts, sessions)
            .map_err(SqliteAccountStateRepositoryError::InvalidState)
    }

    fn store(&self, state: &AccountState) -> Result<(), Self::Error> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteAccountStateRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;

        transaction
            .execute("DELETE FROM account_session", [])
            .map_err(Self::database_error)?;
        transaction
            .execute("DELETE FROM local_account", [])
            .map_err(Self::database_error)?;

        for account in state.accounts().accounts() {
            Self::store_account(&transaction, account)?;
        }

        for session in state.sessions().sessions() {
            Self::store_session(&transaction, session)?;
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

fn parse_status(status: &str) -> Result<AccountStatus, SqliteAccountStateRepositoryError> {
    match status {
        "active" => Ok(AccountStatus::Active),
        "locked" => Ok(AccountStatus::Locked),
        "disabled" => Ok(AccountStatus::Disabled),
        value => Err(SqliteAccountStateRepositoryError::InvalidStatus(
            value.to_owned(),
        )),
    }
}

fn read_timestamp(
    field: &'static str,
    value: i64,
) -> Result<UnixTimestamp, SqliteAccountStateRepositoryError> {
    let value = u64::try_from(value)
        .map_err(|_| SqliteAccountStateRepositoryError::InvalidTimestamp { field, value })?;

    Ok(UnixTimestamp::from_seconds(value))
}

fn read_optional_timestamp(
    field: &'static str,
    value: Option<i64>,
) -> Result<Option<UnixTimestamp>, SqliteAccountStateRepositoryError> {
    value.map(|value| read_timestamp(field, value)).transpose()
}

fn write_timestamp(
    field: &'static str,
    value: UnixTimestamp,
) -> Result<i64, SqliteAccountStateRepositoryError> {
    i64::try_from(value.as_seconds()).map_err(|_| {
        SqliteAccountStateRepositoryError::TimestampOutsideSqliteRange {
            field,
            value: value.as_seconds(),
        }
    })
}

fn write_optional_timestamp(
    field: &'static str,
    value: Option<UnixTimestamp>,
) -> Result<Option<i64>, SqliteAccountStateRepositoryError> {
    value.map(|value| write_timestamp(field, value)).transpose()
}

impl fmt::Display for SqliteAccountStateRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite account repository failed: {error}"),
            Self::InvalidAccount(message) => write!(f, "stored account is invalid: {message}"),
            Self::InvalidSession(message) => {
                write!(f, "stored account session is invalid: {message}")
            }
            Self::InvalidState(error) => write!(f, "stored account state is invalid: {error}"),
            Self::InvalidStatus(status) => write!(f, "stored account status '{status}' is invalid"),
            Self::InvalidTimestamp { field, value } => {
                write!(f, "stored {field} timestamp {value} is invalid")
            }
            Self::TimestampOutsideSqliteRange { field, value } => {
                write!(
                    f,
                    "{field} timestamp {value} is outside SQLite integer range"
                )
            }
            Self::LockPoisoned => write!(f, "SQLite account repository lock is poisoned"),
        }
    }
}

impl Error for SqliteAccountStateRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidState(error) => Some(error),
            Self::InvalidAccount(_)
            | Self::InvalidSession(_)
            | Self::InvalidStatus(_)
            | Self::InvalidTimestamp { .. }
            | Self::TimestampOutsideSqliteRange { .. }
            | Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

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
    fn empty_database_loads_empty_account_state() {
        let repository = SqliteAccountStateRepository::open_in_memory().unwrap();

        let state = repository.load().unwrap();

        assert!(state.accounts().is_empty());
        assert!(state.sessions().is_empty());
    }

    #[test]
    fn multiple_accounts_and_sessions_survive_reopen() {
        let path = database_path("account-round-trip");
        let mut state = AccountState::new();
        let kai = state.create_account("kai", "Kai").unwrap();
        let lea = state.create_account("lea", "Lea").unwrap();
        let kai_session = state
            .start_session(
                &kai,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        let lea_session = state
            .start_session(
                &lea,
                UnixTimestamp::from_seconds(200),
                UnixTimestamp::from_seconds(1_000),
            )
            .unwrap();
        state
            .lock_account(&kai, UnixTimestamp::from_seconds(300))
            .unwrap();

        {
            let repository = SqliteAccountStateRepository::open(&path).unwrap();
            repository.store(&state).unwrap();
        }

        let repository = SqliteAccountStateRepository::open(&path).unwrap();
        let recovered = repository.load().unwrap();

        assert_eq!(recovered.accounts().len(), 2);
        assert_eq!(recovered.sessions().len(), 2);
        assert_eq!(
            recovered.accounts().get(&kai).unwrap().status(),
            AccountStatus::Locked
        );
        assert_eq!(
            recovered.sessions().get(&kai_session).unwrap().revoked_at(),
            Some(UnixTimestamp::from_seconds(300))
        );
        assert!(
            recovered
                .sessions()
                .get(&lea_session)
                .unwrap()
                .revoked_at()
                .is_none()
        );

        drop(repository);
        remove_database(&path);
    }

    #[test]
    fn store_replaces_previous_account_state_atomically() {
        let repository = SqliteAccountStateRepository::open_in_memory().unwrap();
        let mut populated = AccountState::new();
        populated.create_account("kai", "Kai").unwrap();

        repository.store(&populated).unwrap();
        repository.store(&AccountState::new()).unwrap();

        let recovered = repository.load().unwrap();

        assert!(recovered.accounts().is_empty());
        assert!(recovered.sessions().is_empty());
    }
}
