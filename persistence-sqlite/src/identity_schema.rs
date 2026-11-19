use rusqlite::{Connection, OptionalExtension};

pub(super) fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;

         CREATE TABLE IF NOT EXISTS local_account (
             user_id TEXT PRIMARY KEY,
             username TEXT NOT NULL UNIQUE COLLATE NOCASE,
             display_name TEXT NOT NULL,
             status TEXT NOT NULL CHECK (status IN ('active', 'locked', 'disabled'))
         );

         CREATE TABLE IF NOT EXISTS account_role (
             user_id TEXT PRIMARY KEY,
             role TEXT NOT NULL CHECK (role IN ('user', 'manager', 'administrator', 'owner'))
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
         ON account_session(user_id);

         CREATE TABLE IF NOT EXISTS password_credential (
             user_id TEXT PRIMARY KEY,
             password_hash TEXT NOT NULL,
             changed_at INTEGER NOT NULL CHECK (changed_at >= 0),
             failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
             blocked_until INTEGER,
             CHECK (blocked_until IS NULL OR blocked_until >= 0)
         );

         CREATE TABLE IF NOT EXISTS session_credential (
             token_digest BLOB PRIMARY KEY CHECK (length(token_digest) = 32),
             session_id TEXT NOT NULL,
             issued_at INTEGER NOT NULL CHECK (issued_at >= 0),
             revoked_at INTEGER,
             CHECK (revoked_at IS NULL OR revoked_at >= issued_at)
         );

         CREATE INDEX IF NOT EXISTS session_credential_session_id
         ON session_credential(session_id);",
    )?;
    migrate_account_roles(connection)
}

/// Older development databases created `account_role` with a cascading foreign
/// key. Account writes replace rows, which would otherwise erase every role.
fn migrate_account_roles(connection: &Connection) -> rusqlite::Result<()> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'account_role'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(sql) = sql else { return Ok(()) };
    if !sql.contains("REFERENCES local_account") {
        return Ok(());
    }
    connection.execute_batch(
        "DROP TABLE IF EXISTS account_role_migrated;
         CREATE TABLE account_role_migrated (
             user_id TEXT PRIMARY KEY,
             role TEXT NOT NULL CHECK (role IN ('user', 'manager', 'administrator', 'owner'))
         );
         INSERT OR IGNORE INTO account_role_migrated (user_id, role)
             SELECT user_id, role FROM account_role;
         DROP TABLE account_role;
         ALTER TABLE account_role_migrated RENAME TO account_role;",
    )
}
