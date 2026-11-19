use std::error::Error;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, backup::Backup};

/// Rows copied per backup step, bounding how long a source is locked.
const PAGES_PER_STEP: i32 = 256;
const PAUSE_BETWEEN_STEPS: Duration = Duration::from_millis(2);

#[derive(Debug)]
pub enum SqliteBackupError {
    Database(rusqlite::Error),
    Io(std::io::Error),
    InMemory,
}

/// Flushes the write-ahead log into the main database file and `fsync`s it.
///
/// Call this before a slot switch or update so the on-disk database is a single
/// complete file, never a database plus a live WAL.
pub fn checkpoint_and_sync(connection: &Connection) -> Result<(), SqliteBackupError> {
    connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .map_err(SqliteBackupError::Database)?;
    let path = connection.path().ok_or(SqliteBackupError::InMemory)?;
    let path = Path::new(path);
    sync_file(path)?;
    sync_parent(path)
}

/// Copies `source` to `destination` using the SQLite online backup API.
///
/// The result is a consistent snapshot even while writers continue, unlike a
/// file copy which can capture a torn database. The destination is `fsync`ed
/// before the call returns.
pub fn backup_database(source: &Path, destination: &Path) -> Result<(), SqliteBackupError> {
    let source_connection = Connection::open(source).map_err(SqliteBackupError::Database)?;
    let mut destination_connection =
        Connection::open(destination).map_err(SqliteBackupError::Database)?;
    {
        let backup = Backup::new(&source_connection, &mut destination_connection)
            .map_err(SqliteBackupError::Database)?;
        backup
            .run_to_completion(PAGES_PER_STEP, PAUSE_BETWEEN_STEPS, None)
            .map_err(SqliteBackupError::Database)?;
    }
    drop(destination_connection);
    drop(source_connection);
    sync_file(destination)?;
    sync_parent(destination)
}

fn sync_file(path: &Path) -> Result<(), SqliteBackupError> {
    std::fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(SqliteBackupError::Io)
}

fn sync_parent(path: &Path) -> Result<(), SqliteBackupError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(SqliteBackupError::Io)
}

impl fmt::Display for SqliteBackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite backup failed: {error}"),
            Self::Io(error) => write!(f, "SQLite backup I/O failed: {error}"),
            Self::InMemory => f.write_str("cannot checkpoint an in-memory database"),
        }
    }
}

impl Error for SqliteBackupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InMemory => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn backup_captures_committed_rows_without_a_file_copy() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.sqlite");
        let destination = directory.path().join("backup.sqlite");
        {
            let connection = Connection::open(&source).unwrap();
            connection
                .execute_batch(
                    "PRAGMA journal_mode = WAL;
                     CREATE TABLE note (id INTEGER PRIMARY KEY, body TEXT NOT NULL);
                     INSERT INTO note (body) VALUES ('committed');",
                )
                .unwrap();
            checkpoint_and_sync(&connection).unwrap();
        }

        backup_database(&source, &destination).unwrap();

        let backup = Connection::open(&destination).unwrap();
        let body: String = backup
            .query_row("SELECT body FROM note", [], |row| row.get(0))
            .unwrap();
        assert_eq!(body, "committed");

        drop(backup);
        fs::remove_file(source).unwrap();
        fs::remove_file(destination).unwrap();
    }

    #[test]
    fn checkpoint_rejects_in_memory_databases() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(checkpoint_and_sync(&connection).is_err());
    }
}
