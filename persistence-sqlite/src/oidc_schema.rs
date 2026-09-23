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
         ON oidc_client(installation_id);

         CREATE TABLE IF NOT EXISTS oidc_authorization_transaction (
             id TEXT PRIMARY KEY,
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id),
             installation_id TEXT NOT NULL,
             user_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             redirect_uri TEXT NOT NULL,
             scopes TEXT NOT NULL,
             state TEXT NOT NULL,
             nonce TEXT NOT NULL,
             pkce_challenge TEXT NOT NULL,
             expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
             consumed_at INTEGER,
             revoked_at INTEGER
         );

         CREATE TABLE IF NOT EXISTS oidc_authorization_code (
             digest BLOB PRIMARY KEY CHECK (length(digest) = 32),
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id),
             installation_id TEXT NOT NULL,
             user_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             redirect_uri TEXT NOT NULL,
             scopes TEXT NOT NULL,
             nonce TEXT NOT NULL,
             pkce_challenge TEXT NOT NULL,
             expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
             consumed_at INTEGER,
             revoked_at INTEGER
         );

         CREATE TABLE IF NOT EXISTS oidc_consent (
             user_id TEXT NOT NULL,
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id),
             installation_id TEXT NOT NULL,
             scopes TEXT NOT NULL,
             explicit_offline_access INTEGER NOT NULL CHECK (explicit_offline_access IN (0, 1)),
             granted_at INTEGER NOT NULL CHECK (granted_at >= 0),
             revoked_at INTEGER,
             PRIMARY KEY (user_id, client_id)
         );

         CREATE TABLE IF NOT EXISTS oidc_pairwise_subject (
             user_id TEXT NOT NULL,
             installation_id TEXT NOT NULL,
             subject TEXT NOT NULL UNIQUE,
             PRIMARY KEY (user_id, installation_id)
         );

         CREATE TABLE IF NOT EXISTS oidc_token_family (
             id TEXT PRIMARY KEY,
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id),
             installation_id TEXT NOT NULL,
             user_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             current_digest BLOB NOT NULL UNIQUE CHECK (length(current_digest) = 32),
             generation INTEGER NOT NULL CHECK (generation >= 0),
             expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
             revoked_at INTEGER
         );

         CREATE TABLE IF NOT EXISTS oidc_refresh_spent (
             digest BLOB PRIMARY KEY CHECK (length(digest) = 32),
             family_id TEXT NOT NULL REFERENCES oidc_token_family(id)
         );

         CREATE TABLE IF NOT EXISTS oidc_access_token (
             digest BLOB PRIMARY KEY CHECK (length(digest) = 32),
             client_id TEXT NOT NULL REFERENCES oidc_client(client_id),
             installation_id TEXT NOT NULL,
             user_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             subject TEXT NOT NULL,
             scopes TEXT NOT NULL,
             family_id TEXT REFERENCES oidc_token_family(id),
             expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
             revoked_at INTEGER
         );

         CREATE INDEX IF NOT EXISTS oidc_transaction_installation
         ON oidc_authorization_transaction(installation_id);
         CREATE INDEX IF NOT EXISTS oidc_code_installation
         ON oidc_authorization_code(installation_id);
         CREATE INDEX IF NOT EXISTS oidc_family_installation
         ON oidc_token_family(installation_id);
         CREATE INDEX IF NOT EXISTS oidc_access_installation
         ON oidc_access_token(installation_id);",
    )
}
