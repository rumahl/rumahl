use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{InstallationId, OidcScope, SessionId, UnixTimestamp, UserId};
use rumahl_oidc_provider::{AccessTokenGrant, OidcAccessTokenStore, OidcClientId, PairwiseSubject};
use rusqlite::{Connection, OptionalExtension, params};

use crate::oidc_schema;

pub struct SqliteOidcAccessTokenStore {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteOidcAccessTokenStoreError {
    Database(rusqlite::Error),
    InvalidStoredState,
    InactiveClient,
    LockPoisoned,
}

impl fmt::Display for SqliteOidcAccessTokenStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OIDC access-token storage failed")
    }
}

impl Error for SqliteOidcAccessTokenStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl SqliteOidcAccessTokenStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteOidcAccessTokenStoreError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteOidcAccessTokenStoreError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteOidcAccessTokenStoreError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        oidc_schema::initialize(&connection).map_err(Self::database_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteOidcAccessTokenStoreError {
        SqliteOidcAccessTokenStoreError::Database(error)
    }
}

impl OidcAccessTokenStore for SqliteOidcAccessTokenStore {
    type Error = SqliteOidcAccessTokenStoreError;

    fn insert_access(
        &self,
        grant: &AccessTokenGrant,
        family_id: Option<&str>,
    ) -> Result<(), Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let inserted = connection.execute(
            "INSERT INTO oidc_access_token
             (digest, client_id, installation_id, user_id, session_id, subject, scopes, family_id, expires_at)
             SELECT ?1, c.client_id, c.installation_id, ?3, ?4, ?5, ?6, ?7, ?8
             FROM oidc_client c
             WHERE c.client_id = ?2 AND c.installation_id = ?9 AND c.revoked_at IS NULL
               AND EXISTS (SELECT 1 FROM oidc_pairwise_subject s
                    WHERE s.user_id = ?3 AND s.installation_id = ?9 AND s.subject = ?5)
               AND (?7 IS NULL OR EXISTS (
                    SELECT 1 FROM oidc_token_family f WHERE f.id = ?7
                      AND f.client_id = ?2 AND f.user_id = ?3 AND f.session_id = ?4
                      AND f.revoked_at IS NULL))",
            params![&grant.digest()[..], grant.client_id().as_str(), grant.user_id().to_string(),
                grant.session_id().to_string(), grant.subject().value(),
                grant.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>().join(" "),
                family_id, timestamp(grant.expires_at())?, grant.subject().installation_id().to_string()],
        ).map_err(Self::database_error)?;
        if inserted != 1 {
            return Err(Self::Error::InactiveClient);
        }
        Ok(())
    }

    fn resolve_access(
        &self,
        raw: &str,
        now: UnixTimestamp,
    ) -> Result<Option<AccessTokenGrant>, Self::Error> {
        if raw.len() != 43 {
            return Ok(None);
        }
        let digest = AccessTokenGrant::digest_raw(raw);
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let stored = connection.query_row(
            "SELECT a.client_id, a.installation_id, a.user_id, a.session_id, a.subject, a.scopes, a.expires_at
             FROM oidc_access_token a
             JOIN oidc_client c ON c.client_id = a.client_id
               AND c.installation_id = a.installation_id
             JOIN oidc_pairwise_subject s ON s.user_id = a.user_id
               AND s.installation_id = a.installation_id AND s.subject = a.subject
             LEFT JOIN oidc_token_family f ON f.id = a.family_id
             WHERE a.digest = ?1 AND a.revoked_at IS NULL AND c.revoked_at IS NULL
               AND a.expires_at > ?2
               AND (a.family_id IS NULL OR (f.revoked_at IS NULL AND f.expires_at > ?2
                    AND f.client_id = a.client_id AND f.installation_id = a.installation_id
                    AND f.user_id = a.user_id AND f.session_id = a.session_id))",
            params![&digest[..], timestamp(now)?],
            |row| Ok(StoredAccess {
                client_id: row.get(0)?, installation_id: row.get(1)?, user_id: row.get(2)?,
                session_id: row.get(3)?, subject: row.get(4)?, scopes: row.get(5)?, expires_at: row.get(6)?,
            }),
        ).optional().map_err(Self::database_error)?;
        stored.map(|stored| stored.into_domain(digest)).transpose()
    }

    fn revoke_access(&self, raw: &str, now: UnixTimestamp) -> Result<usize, Self::Error> {
        if raw.len() != 43 {
            return Ok(0);
        }
        let digest = AccessTokenGrant::digest_raw(raw);
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        connection.execute(
            "UPDATE oidc_access_token SET revoked_at = ?2 WHERE digest = ?1 AND revoked_at IS NULL",
            params![&digest[..], timestamp(now)?],
        ).map_err(Self::database_error)
    }
}

struct StoredAccess {
    client_id: String,
    installation_id: String,
    user_id: String,
    session_id: String,
    subject: String,
    scopes: String,
    expires_at: i64,
}

impl StoredAccess {
    fn into_domain(
        self,
        digest: [u8; 32],
    ) -> Result<AccessTokenGrant, SqliteOidcAccessTokenStoreError> {
        let user_id = UserId::parse(&self.user_id)
            .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?;
        let installation_id = InstallationId::parse(&self.installation_id)
            .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?;
        let subject = PairwiseSubject::restore(user_id, installation_id, self.subject)
            .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?;
        let scopes = self
            .scopes
            .split(' ')
            .map(|scope| {
                OidcScope::parse(scope).ok_or(SqliteOidcAccessTokenStoreError::InvalidStoredState)
            })
            .collect::<Result<Vec<_>, _>>()?;
        AccessTokenGrant::restore(
            digest,
            OidcClientId::parse(self.client_id)
                .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?,
            user_id,
            SessionId::parse(&self.session_id)
                .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?,
            subject,
            scopes,
            u64::try_from(self.expires_at)
                .map(UnixTimestamp::from_seconds)
                .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)?,
        )
        .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)
    }
}

fn timestamp(value: UnixTimestamp) -> Result<i64, SqliteOidcAccessTokenStoreError> {
    i64::try_from(value.as_seconds())
        .map_err(|_| SqliteOidcAccessTokenStoreError::InvalidStoredState)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{AppId, OidcClientType};
    use rumahl_oidc_provider::{OidcClientRecord, OidcClientRepository, OidcRedirectUri};

    use crate::SqliteOidcClientRepository;

    #[test]
    fn opaque_access_token_is_digest_only_and_revocable_after_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("access.sqlite3");
        let client = OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Public,
            OidcRedirectUri::parse("https://cloud.rumahl.dev/oidc/callback").unwrap(),
            vec![OidcScope::OpenId],
            None,
            UnixTimestamp::from_seconds(100),
            None,
        )
        .unwrap();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let user_id = UserId::new();
        let subject = PairwiseSubject::generate(user_id, *client.installation_id()).unwrap();
        let store = SqliteOidcAccessTokenStore::open(&path).unwrap();
        store.connection.lock().unwrap().execute(
            "INSERT INTO oidc_pairwise_subject (user_id, installation_id, subject) VALUES (?1, ?2, ?3)",
            params![user_id.to_string(), client.installation_id().to_string(), subject.value()],
        ).unwrap();
        let (grant, raw) = AccessTokenGrant::issue(
            client.client_id().clone(),
            user_id,
            SessionId::new(),
            subject,
            vec![OidcScope::OpenId],
            UnixTimestamp::from_seconds(200),
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
        store.insert_access(&grant, None).unwrap();
        drop(store);
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(raw.len())
                .any(|window| window == raw.as_bytes())
        );
        let store = SqliteOidcAccessTokenStore::open(&path).unwrap();
        assert_eq!(
            store
                .resolve_access(&raw, UnixTimestamp::from_seconds(150))
                .unwrap(),
            Some(grant)
        );
        assert!(
            store
                .resolve_access(&raw, UnixTimestamp::from_seconds(200))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .revoke_access(&raw, UnixTimestamp::from_seconds(160))
                .unwrap(),
            1
        );
        assert!(
            store
                .resolve_access(&raw, UnixTimestamp::from_seconds(161))
                .unwrap()
                .is_none()
        );
    }
}
