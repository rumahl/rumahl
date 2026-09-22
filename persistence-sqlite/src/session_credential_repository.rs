use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_account_auth::{
    SessionCredentialRecord, SessionCredentialRepository, SessionTokenDigest,
};
use rumahl_core::{SessionId, UnixTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::identity_schema;

#[derive(Debug)]
pub struct SqliteSessionCredentialRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteSessionCredentialRepositoryError {
    Database(rusqlite::Error),
    InvalidSessionId(String),
    TimestampOutsideSqliteRange { field: &'static str, value: u64 },
    LockPoisoned,
}

impl SqliteSessionCredentialRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteSessionCredentialRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteSessionCredentialRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(
        connection: Connection,
    ) -> Result<Self, SqliteSessionCredentialRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        identity_schema::initialize(&connection).map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteSessionCredentialRepositoryError {
        SqliteSessionCredentialRepositoryError::Database(error)
    }
}

impl SessionCredentialRepository for SqliteSessionCredentialRepository {
    type Error = SqliteSessionCredentialRepositoryError;

    fn insert(&self, credential: &SessionCredentialRecord) -> Result<(), Self::Error> {
        let issued_at = write_timestamp("issued_at", credential.issued_at())?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteSessionCredentialRepositoryError::LockPoisoned)?;

        connection
            .execute(
                "INSERT INTO session_credential (
                     token_digest, session_id, issued_at, revoked_at
                 ) VALUES (?1, ?2, ?3, NULL)",
                params![
                    &credential.digest().as_bytes()[..],
                    credential.session_id().to_string(),
                    issued_at,
                ],
            )
            .map_err(Self::database_error)?;

        Ok(())
    }

    fn resolve(&self, digest: &SessionTokenDigest) -> Result<Option<SessionId>, Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteSessionCredentialRepositoryError::LockPoisoned)?;
        let session_id = connection
            .query_row(
                "SELECT session_id
                 FROM session_credential
                 WHERE token_digest = ?1 AND revoked_at IS NULL",
                [&digest.as_bytes()[..]],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Self::database_error)?;

        session_id
            .map(|session_id| {
                SessionId::parse(&session_id).map_err(|_| {
                    SqliteSessionCredentialRepositoryError::InvalidSessionId(session_id)
                })
            })
            .transpose()
    }

    fn revoke_session(
        &self,
        session_id: &SessionId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, Self::Error> {
        let revoked_at = write_timestamp("revoked_at", revoked_at)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteSessionCredentialRepositoryError::LockPoisoned)?;

        connection
            .execute(
                "UPDATE session_credential
                 SET revoked_at = ?2
                 WHERE session_id = ?1 AND revoked_at IS NULL",
                params![session_id.to_string(), revoked_at],
            )
            .map_err(Self::database_error)
    }
}

fn write_timestamp(
    field: &'static str,
    value: UnixTimestamp,
) -> Result<i64, SqliteSessionCredentialRepositoryError> {
    i64::try_from(value.as_seconds()).map_err(|_| {
        SqliteSessionCredentialRepositoryError::TimestampOutsideSqliteRange {
            field,
            value: value.as_seconds(),
        }
    })
}

impl fmt::Display for SqliteSessionCredentialRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => {
                write!(f, "SQLite session credential repository failed: {error}")
            }
            Self::InvalidSessionId(session_id) => {
                write!(f, "stored session ID '{session_id}' is invalid")
            }
            Self::TimestampOutsideSqliteRange { field, value } => {
                write!(
                    f,
                    "{field} timestamp {value} is outside SQLite integer range"
                )
            }
            Self::LockPoisoned => {
                write!(f, "SQLite session credential repository lock is poisoned")
            }
        }
    }
}

impl Error for SqliteSessionCredentialRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidSessionId(_)
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
        SessionCredentialIssuer, SessionToken, StoredSessionCredentialResolver,
    };
    use rumahl_platform_api::SessionCredentialResolver;

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
    fn stores_only_digest_and_resolves_opaque_token() {
        let repository = SqliteSessionCredentialRepository::open_in_memory().unwrap();
        let session_id = SessionId::new();
        let token = SessionToken::generate().unwrap();
        let encoded = token.encode();
        let record = SessionCredentialRecord::new(
            token.digest(),
            session_id,
            UnixTimestamp::from_seconds(100),
        );

        repository.insert(&record).unwrap();

        let resolver = StoredSessionCredentialResolver::new(repository);

        assert_eq!(resolver.resolve_session(&encoded).unwrap(), session_id);
    }

    #[test]
    fn credential_survives_reopen_and_revocation() {
        let path = database_path("session-credential");
        let session_id = SessionId::new();
        let encoded;

        {
            let issuer = SessionCredentialIssuer::new(
                SqliteSessionCredentialRepository::open(&path).unwrap(),
            );
            let token = issuer
                .issue(session_id, UnixTimestamp::from_seconds(100))
                .unwrap();
            encoded = token.encode();
        }

        let repository = SqliteSessionCredentialRepository::open(&path).unwrap();

        assert_eq!(
            repository
                .resolve(&SessionToken::parse(&encoded).unwrap().digest())
                .unwrap(),
            Some(session_id)
        );
        assert_eq!(
            repository
                .revoke_session(&session_id, UnixTimestamp::from_seconds(150))
                .unwrap(),
            1
        );
        assert_eq!(
            repository
                .resolve(&SessionToken::parse(&encoded).unwrap().digest())
                .unwrap(),
            None
        );

        drop(repository);
        remove_database(&path);
    }
}
