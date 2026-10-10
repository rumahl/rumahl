//! Database schema versioning.
//!
//! Every rumahl SQLite database records its schema version in `PRAGMA
//! user_version`. On startup the platform stamps a fresh (or pre-versioning)
//! database with the version this binary supports and refuses to run against a
//! database written by a newer binary. That is what keeps an A/B slot rollback
//! from corrupting a database that a newer slot has already migrated.

use rusqlite::Connection;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::time::Duration;

/// Schema version understood by this binary.
///
/// Bump this together with the repository migration that introduces a change.
pub const SCHEMA_VERSION: i64 = 2;

const BUSY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub enum SchemaVersionError {
    /// The database was written by a newer binary; start it with that binary.
    Newer {
        found: i64,
        supported: i64,
    },
    Storage(rusqlite::Error),
}

/// Verifies (and stamps) the schema version of a database file.
pub fn verify_schema(path: impl AsRef<Path>) -> Result<(), SchemaVersionError> {
    let connection = Connection::open(path).map_err(SchemaVersionError::Storage)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(SchemaVersionError::Storage)?;
    verify_connection(&connection)
}

/// Verifies the schema version on an open connection.
///
/// A version below the current one is stamped forward: repositories create
/// their tables and add columns idempotently, so the caller can adopt the new
/// shape. A version above the current one is refused.
pub fn verify_connection(connection: &Connection) -> Result<(), SchemaVersionError> {
    let found: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SchemaVersionError::Storage)?;
    if found > SCHEMA_VERSION {
        return Err(SchemaVersionError::Newer {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found < SCHEMA_VERSION {
        connection
            .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .map_err(SchemaVersionError::Storage)?;
    }
    Ok(())
}

/// Adds a column to an existing table exactly once.
///
/// This helper builds DDL by interpolation, so the identifiers are restricted
/// to a safe set even though every caller passes a compile-time constant;
/// `definition` stays an internal constant.
pub(crate) fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), rusqlite::Error> {
    if !valid_identifier(table) || !valid_identifier(column) {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid SQL identifier for {table}.{column}"
        )));
    }
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|name| name == column) {
        connection.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl fmt::Display for SchemaVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Newer { found, supported } => write!(
                f,
                "database schema version {found} is newer than the supported version {supported}"
            ),
            Self::Storage(error) => write!(f, "database schema check failed: {error}"),
        }
    }
}

impl Error for SchemaVersionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Newer { .. } => None,
            Self::Storage(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_a_fresh_database_and_accepts_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("database.sqlite");
        verify_schema(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        // Reopening the same database is accepted.
        verify_schema(&path).unwrap();
    }

    #[test]
    fn refuses_a_newer_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("database.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1))
            .unwrap();
        drop(connection);
        assert!(matches!(
            verify_schema(&path).unwrap_err(),
            SchemaVersionError::Newer { .. }
        ));
    }
}
