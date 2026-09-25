//! Atomic browser session creation and revocation; never rewrites account snapshots.
use rumahl_account_auth::{SessionToken, VerifiedLocalAccount};
use rumahl_core::{SessionId, UnixTimestamp};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, sync::Mutex, time::Duration};

pub struct SqliteBrowserSessionRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum BrowserSessionError {
    Storage,
    InvalidProof,
}
impl std::fmt::Display for BrowserSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("browser session operation failed")
    }
}
impl std::error::Error for BrowserSessionError {}

impl SqliteBrowserSessionRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BrowserSessionError> {
        let connection = Connection::open(path).map_err(|_| BrowserSessionError::Storage)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| BrowserSessionError::Storage)?;
        crate::identity_schema::initialize(&connection)
            .map_err(|_| BrowserSessionError::Storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn create(
        &self,
        proof: VerifiedLocalAccount,
        expires_at: UnixTimestamp,
    ) -> Result<SessionToken, BrowserSessionError> {
        let at = i64::try_from(proof.authenticated_at().as_seconds())
            .map_err(|_| BrowserSessionError::InvalidProof)?;
        let expiry = i64::try_from(expires_at.as_seconds())
            .map_err(|_| BrowserSessionError::InvalidProof)?;
        if expiry <= at {
            return Err(BrowserSessionError::InvalidProof);
        }
        let token = SessionToken::generate().map_err(|_| BrowserSessionError::Storage)?;
        let session = SessionId::new();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrowserSessionError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| BrowserSessionError::Storage)?;
        // Lock and compare both account status and exact verifier in the same
        // transaction that publishes the session and its credential digest.
        let valid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM local_account a JOIN password_credential p USING(user_id)
             WHERE a.user_id = ?1 AND a.status = 'active' AND p.password_hash = ?2)",
            params![proof.user_id().to_string(), proof.password_hash().as_str()], |row| row.get(0)
        ).map_err(|_| BrowserSessionError::Storage)?;
        if !valid {
            return Err(BrowserSessionError::InvalidProof);
        }
        tx.execute("INSERT INTO account_session
            (session_id, user_id, authenticated_at, last_seen_at, reauthenticated_at, expires_at, revoked_at)
            VALUES (?1, ?2, ?3, ?3, ?3, ?4, NULL)",
            params![session.to_string(), proof.user_id().to_string(), at, expiry]).map_err(|_| BrowserSessionError::Storage)?;
        tx.execute(
            "INSERT INTO session_credential (token_digest, session_id, issued_at, revoked_at)
            VALUES (?1, ?2, ?3, NULL)",
            params![&token.digest().as_bytes()[..], session.to_string(), at],
        )
        .map_err(|_| BrowserSessionError::Storage)?;
        tx.commit().map_err(|_| BrowserSessionError::Storage)?;
        Ok(token)
    }

    /// Idempotently revoke the OS session as well as its credentials. OIDC and
    /// WebSocket authentication observe the same durable session revocation.
    pub fn revoke(
        &self,
        token: &SessionToken,
        now: UnixTimestamp,
    ) -> Result<(), BrowserSessionError> {
        let at = i64::try_from(now.as_seconds()).map_err(|_| BrowserSessionError::Storage)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrowserSessionError::Storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| BrowserSessionError::Storage)?;
        let session: Option<String> = tx
            .query_row(
                "SELECT session_id FROM session_credential WHERE token_digest = ?1",
                [&token.digest().as_bytes()[..]],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| BrowserSessionError::Storage)?;
        if let Some(session) = session {
            tx.execute(
                "UPDATE account_session SET revoked_at = MAX(?2, authenticated_at)
                WHERE session_id = ?1 AND revoked_at IS NULL",
                params![session, at],
            )
            .map_err(|_| BrowserSessionError::Storage)?;
            tx.execute(
                "UPDATE session_credential SET revoked_at = MAX(?2, issued_at)
                WHERE session_id = ?1 AND revoked_at IS NULL",
                params![session, at],
            )
            .map_err(|_| BrowserSessionError::Storage)?;
        }
        tx.commit().map_err(|_| BrowserSessionError::Storage)
    }
}
