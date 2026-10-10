use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{
    AuditAction, AuditActor, AuditError, AuditEvent, AuditLog, AuditOutcome, NewAuditEvent,
    UnixTimestamp, UserId,
};
use rusqlite::{Connection, params};

/// The largest `recent` query the adapter will serve.
pub const MAX_RECENT_EVENTS: usize = 1000;

/// Append-only SQLite audit log.
///
/// Updates and deletes are rejected by database triggers, so history cannot be
/// rewritten through this adapter even if application code is compromised.
/// Secrets are never written here; only bounded, non-secret targets.
pub struct SqliteAuditLog {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteAuditLogError {
    Database(rusqlite::Error),
    LockPoisoned,
}

impl SqliteAuditLog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteAuditLogError> {
        Self::from_connection(Connection::open(path).map_err(SqliteAuditLogError::Database)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteAuditLogError> {
        Self::from_connection(Connection::open_in_memory().map_err(SqliteAuditLogError::Database)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteAuditLogError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(SqliteAuditLogError::Database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS audit_event (
                     sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                     at INTEGER NOT NULL CHECK (at >= 0),
                     actor TEXT NOT NULL,
                     action TEXT NOT NULL CHECK (
                         action IN (
                             'sign-in', 'sign-in-failed', 'sign-out',
                             'reauthenticate', 'reauthenticate-failed', 'os-mode-changed'
                         )
                     ),
                     target TEXT,
                     outcome TEXT NOT NULL CHECK (
                         outcome IN ('success', 'denied', 'failure')
                     )
                 );

                 CREATE INDEX IF NOT EXISTS audit_event_actor
                 ON audit_event(actor, sequence);

                 CREATE TRIGGER IF NOT EXISTS audit_event_append_only_update
                 BEFORE UPDATE ON audit_event
                 BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;

                 CREATE TRIGGER IF NOT EXISTS audit_event_append_only_delete
                 BEFORE DELETE ON audit_event
                 BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;",
            )
            .map_err(SqliteAuditLogError::Database)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqliteAuditLogError> {
        self.connection
            .lock()
            .map_err(|_| SqliteAuditLogError::LockPoisoned)
    }
}

impl AuditLog for SqliteAuditLog {
    fn record(&self, event: NewAuditEvent) -> Result<u64, AuditError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let at = i64::try_from(event.at.as_seconds()).map_err(|_| AuditError::Unavailable)?;
        connection
            .execute(
                "INSERT INTO audit_event (at, actor, action, target, outcome)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    at,
                    event.actor.encode(),
                    event.action.as_str(),
                    event.target,
                    event.outcome.as_str(),
                ],
            )
            .map_err(|_| AuditError::Unavailable)?;
        u64::try_from(connection.last_insert_rowid()).map_err(|_| AuditError::Unavailable)
    }

    fn recent(&self, actor: UserId, limit: usize) -> Result<Vec<AuditEvent>, AuditError> {
        let limit =
            i64::try_from(limit.min(MAX_RECENT_EVENTS)).map_err(|_| AuditError::Unavailable)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let mut statement = connection
            .prepare(
                "SELECT sequence, at, actor, action, target, outcome
                 FROM audit_event
                 WHERE actor = ?1
                 ORDER BY sequence DESC
                 LIMIT ?2",
            )
            .map_err(|_| AuditError::Unavailable)?;
        let rows = statement
            .query_map(params![AuditActor::User(actor).encode(), limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|_| AuditError::Unavailable)?;
        let mut events = Vec::new();
        for row in rows {
            let (sequence, at, actor, action, target, outcome) =
                row.map_err(|_| AuditError::Unavailable)?;
            events.push(AuditEvent {
                sequence: u64::try_from(sequence).map_err(|_| AuditError::Unavailable)?,
                at: UnixTimestamp::from_seconds(
                    u64::try_from(at).map_err(|_| AuditError::Unavailable)?,
                ),
                actor: AuditActor::decode(&actor)?,
                action: AuditAction::parse(&action)?,
                outcome: AuditOutcome::parse(&outcome)?,
                target,
            });
        }
        Ok(events)
    }
}

impl fmt::Display for SqliteAuditLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite audit log failed: {error}"),
            Self::LockPoisoned => f.write_str("audit log lock is poisoned"),
        }
    }
}

impl Error for SqliteAuditLogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(actor: AuditActor, action: AuditAction, outcome: AuditOutcome) -> NewAuditEvent {
        NewAuditEvent::new(UnixTimestamp::from_seconds(10), actor, action, outcome)
    }

    #[test]
    fn appends_and_reads_only_the_users_own_events_newest_first() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let user = UserId::new();
        let other = UserId::new();
        log.record(event(
            AuditActor::User(user),
            AuditAction::SignIn,
            AuditOutcome::Success,
        ))
        .unwrap();
        log.record(event(
            AuditActor::User(other),
            AuditAction::SignIn,
            AuditOutcome::Success,
        ))
        .unwrap();
        log.record(event(
            AuditActor::User(user),
            AuditAction::OsModeChanged,
            AuditOutcome::Success,
        ))
        .unwrap();

        let events = log.recent(user, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, AuditAction::OsModeChanged);
        assert_eq!(events[1].action, AuditAction::SignIn);
        assert!(events[0].sequence > events[1].sequence);
    }

    #[test]
    fn history_cannot_be_updated_or_deleted() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let user = UserId::new();
        log.record(event(
            AuditActor::User(user),
            AuditAction::SignIn,
            AuditOutcome::Success,
        ))
        .unwrap();

        let connection = log.connection().unwrap();
        assert!(
            connection
                .execute("UPDATE audit_event SET outcome = 'failure'", [])
                .is_err()
        );
        assert!(connection.execute("DELETE FROM audit_event", []).is_err());
        drop(connection);

        assert_eq!(log.recent(user, 10).unwrap().len(), 1);
    }

    #[test]
    fn records_bounded_non_secret_targets() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let user = UserId::new();
        let event = event(
            AuditActor::User(user),
            AuditAction::OsModeChanged,
            AuditOutcome::Success,
        )
        .with_target("advanced")
        .unwrap();
        log.record(event).unwrap();

        let events = log.recent(user, 10).unwrap();
        assert_eq!(events[0].target.as_deref(), Some("advanced"));
    }
}
