use rusqlite::Connection;

pub(super) fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;

         CREATE TABLE IF NOT EXISTS oidc_client (
             client_id TEXT PRIMARY KEY,
             installation_id TEXT NOT NULL UNIQUE,
             app_id TEXT NOT NULL,
             display_name TEXT NOT NULL,
             client_type TEXT NOT NULL CHECK (client_type IN ('public', 'confidential')),
             redirect_uri TEXT NOT NULL,
             client_secret_digest BLOB,
             created_at INTEGER NOT NULL CHECK (created_at >= 0),
             revoked_at INTEGER,
             CHECK (revoked_at IS NULL OR revoked_at >= created_at),
             CHECK (
                 (client_type = 'public' AND client_secret_digest IS NULL) OR
                 (client_type = 'confidential' AND length(client_secret_digest) = 32)
             )
         );

         CREATE TABLE IF NOT EXISTS oidc_client_scope (
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id) ON DELETE CASCADE,
             scope TEXT NOT NULL CHECK (
                 scope IN ('openid', 'profile', 'email', 'offline_access')
             ),
             PRIMARY KEY (client_id, scope)
         );

         CREATE INDEX IF NOT EXISTS oidc_client_installation_id
         ON oidc_client(installation_id);",
    )
}
