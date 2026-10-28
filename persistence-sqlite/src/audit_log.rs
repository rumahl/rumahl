use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{
    AuditAction, AuditActor, AuditError, AuditEvent, AuditLog, AuditOutcome, AuditQuery,
    NewAuditEvent, UnixTimestamp, UserId,
};
use rusqlite::{Connection, OptionalExtension, params};

/// The largest `recent*` query the adapter will serve.
pub const MAX_RECENT_EVENTS: usize = 1000;

/// Column list shared by every read query (keeps the mapping in one place).
const EVENT_COLUMNS: &str = "sequence, at, actor, action, target, outcome";

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
                 PRAGMA synchronous = FULL;",
            )
            .map_err(SqliteAuditLogError::Database)?;
        migrate(&connection)?;
        connection
            .execute_batch(
                // The action set is validated by the domain (`AuditAction`), so
                // new actions never need a destructive CHECK migration.
                "CREATE TABLE IF NOT EXISTS audit_event (
                     sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                     at INTEGER NOT NULL CHECK (at >= 0),
                     actor TEXT NOT NULL,
                     action TEXT NOT NULL,
                     target TEXT,
                     outcome TEXT NOT NULL CHECK (
                         outcome IN ('success', 'denied', 'failure')
                     )
                 );

                 CREATE INDEX IF NOT EXISTS audit_event_actor
                 ON audit_event(actor, sequence);

                 CREATE INDEX IF NOT EXISTS audit_event_target
                 ON audit_event(target, sequence);

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

/// Rewrites an early audit table whose `action` column carried a fixed CHECK
/// constraint, so new actions can be recorded without losing history.
fn migrate(connection: &Connection) -> Result<(), SqliteAuditLogError> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'audit_event'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(SqliteAuditLogError::Database)?;
    let Some(sql) = sql else { return Ok(()) };
    if !sql.contains("action IN") {
        return Ok(());
    }
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS audit_event_append_only_update;
             DROP TRIGGER IF EXISTS audit_event_append_only_delete;
             ALTER TABLE audit_event RENAME TO audit_event_legacy;
             CREATE TABLE audit_event (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 at INTEGER NOT NULL CHECK (at >= 0),
                 actor TEXT NOT NULL,
                 action TEXT NOT NULL,
                 target TEXT,
                 outcome TEXT NOT NULL CHECK (
                     outcome IN ('success', 'denied', 'failure')
                 )
             );
             INSERT INTO audit_event (sequence, at, actor, action, target, outcome)
                 SELECT sequence, at, actor, action, target, outcome FROM audit_event_legacy;
             DROP TABLE audit_event_legacy;",
        )
        .map_err(SqliteAuditLogError::Database)?;
    Ok(())
}

fn decode(
    row: (i64, i64, String, String, Option<String>, String),
) -> Result<AuditEvent, AuditError> {
    let (sequence, at, actor, action, target, outcome) = row;
    Ok(AuditEvent {
        sequence: u64::try_from(sequence).map_err(|_| AuditError::Unavailable)?,
        at: UnixTimestamp::from_seconds(u64::try_from(at).map_err(|_| AuditError::Unavailable)?),
        actor: AuditActor::decode(&actor)?,
        action: AuditAction::parse(&action)?,
        outcome: AuditOutcome::parse(&outcome)?,
        target,
    })
}

type RawRow = (i64, i64, String, String, Option<String>, String);

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn limit_of(limit: usize) -> Result<i64, AuditError> {
    i64::try_from(limit.min(MAX_RECENT_EVENTS)).map_err(|_| AuditError::Unavailable)
}

fn finish<I: Iterator<Item = rusqlite::Result<RawRow>>>(
    rows: I,
) -> Result<Vec<AuditEvent>, AuditError> {
    let mut events = Vec::new();
    for row in rows {
        events.push(decode(row.map_err(|_| AuditError::Unavailable)?)?);
    }
    Ok(events)
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
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM audit_event WHERE actor = ?1 ORDER BY sequence DESC LIMIT ?2"
            ))
            .map_err(|_| AuditError::Unavailable)?;
        let rows = statement
            .query_map(
                params![AuditActor::User(actor).encode(), limit_of(limit)?],
                map_row,
            )
            .map_err(|_| AuditError::Unavailable)?;
        finish(rows)
    }

    fn recent_system(&self, limit: usize) -> Result<Vec<AuditEvent>, AuditError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM audit_event ORDER BY sequence DESC LIMIT ?1"
            ))
            .map_err(|_| AuditError::Unavailable)?;
        let rows = statement
            .query_map(params![limit_of(limit)?], map_row)
            .map_err(|_| AuditError::Unavailable)?;
        finish(rows)
    }

    fn recent_target(&self, target: &str, limit: usize) -> Result<Vec<AuditEvent>, AuditError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM audit_event WHERE target = ?1 ORDER BY sequence DESC LIMIT ?2"
            ))
            .map_err(|_| AuditError::Unavailable)?;
        let rows = statement
            .query_map(params![target, limit_of(limit)?], map_row)
            .map_err(|_| AuditError::Unavailable)?;
        finish(rows)
    }

    fn query_system(&self, query: &AuditQuery) -> Result<Vec<AuditEvent>, AuditError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AuditError::Unavailable)?;
        let mut sql = String::from(
            "SELECT sequence, at, actor, action, target, outcome FROM audit_event WHERE 1 = 1",
        );
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(action) = &query.action {
            sql.push_str(" AND action = ?");
            values.push(Box::new(action.as_str().to_owned()));
        }
        if let Some(actor) = &query.actor {
            sql.push_str(" AND actor = ?");
            values.push(Box::new(actor.encode()));
        }
        if let Some(outcome) = &query.outcome {
            sql.push_str(" AND outcome = ?");
            values.push(Box::new(outcome.as_str().to_owned()));
        }
        if let Some(target) = &query.target {
            sql.push_str(" AND target = ?");
            values.push(Box::new(target.clone()));
        }
        if let Some(since) = query.since {
            sql.push_str(" AND at >= ?");
            values.push(Box::new(
                i64::try_from(since.as_seconds()).map_err(|_| AuditError::Unavailable)?,
            ));
        }
        sql.push_str(" ORDER BY sequence DESC LIMIT ? OFFSET ?");
        values.push(Box::new(limit_of(query.limit)?));
        values.push(Box::new(
            i64::try_from(query.offset.min(1_000_000)).map_err(|_| AuditError::Unavailable)?,
        ));
        let mut statement = connection
            .prepare(&sql)
            .map_err(|_| AuditError::Unavailable)?;
        let rows = statement
            .query_map(
                rusqlite::params_from_iter(values.iter().map(|value| value.as_ref())),
                map_row,
            )
            .map_err(|_| AuditError::Unavailable)?;
        finish(rows)
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
    fn records_and_reads_system_wide_events_from_every_actor() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let user = UserId::new();
        log.record(event(
            AuditActor::User(user),
            AuditAction::AppLaunched,
            AuditOutcome::Success,
        ))
        .unwrap();
        log.record(event(
            AuditActor::App("com.example.notes".to_owned()),
            AuditAction::AppCrashed,
            AuditOutcome::Failure,
        ))
        .unwrap();
        log.record(event(
            AuditActor::Service("renderer".to_owned()),
            AuditAction::ServiceStale,
            AuditOutcome::Failure,
        ))
        .unwrap();

        let events = log.recent_system(10).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].action, AuditAction::ServiceStale);
        assert_eq!(events[1].action, AuditAction::AppCrashed);
        assert_eq!(events[2].action, AuditAction::AppLaunched);

        let crashed = log.recent_target("com.example.notes", 10).unwrap();
        assert_eq!(crashed.len(), 0);
    }

    #[test]
    fn filters_by_target() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let event = event(
            AuditActor::App("com.example.notes".to_owned()),
            AuditAction::AppCrashed,
            AuditOutcome::Failure,
        )
        .with_target("com.example.notes")
        .unwrap();
        log.record(event).unwrap();
        assert_eq!(log.recent_target("com.example.notes", 10).unwrap().len(), 1);
        assert_eq!(log.recent_target("other", 10).unwrap().len(), 0);
    }

    #[test]
    fn filters_system_events_by_actor_action_and_outcome() {
        let log = SqliteAuditLog::open_in_memory().unwrap();
        let user = UserId::new();
        log.record(event(
            AuditActor::User(user),
            AuditAction::SignIn,
            AuditOutcome::Success,
        ))
        .unwrap();
        log.record(event(
            AuditActor::App("com.x".to_owned()),
            AuditAction::AppCrashed,
            AuditOutcome::Failure,
        ))
        .unwrap();
        log.record(event(
            AuditActor::App("com.x".to_owned()),
            AuditAction::AppLaunched,
            AuditOutcome::Success,
        ))
        .unwrap();

        let failures = log
            .query_system(&AuditQuery {
                outcome: Some(AuditOutcome::Failure),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].action, AuditAction::AppCrashed);

        let app = AuditActor::App("com.x".to_owned());
        let by_actor = log
            .query_system(&AuditQuery {
                actor: Some(app),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_actor.len(), 2);

        let crashed = log
            .query_system(&AuditQuery {
                action: Some(AuditAction::AppCrashed),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(crashed.len(), 1);
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
