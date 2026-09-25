use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{SessionId, UnixTimestamp, UserId};
use rumahl_oidc_provider::{
    OidcClientId, OidcRefreshTokenStore, RefreshRotation, RefreshTokenFamily,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::oidc_schema;

pub struct SqliteOidcRefreshTokenStore {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteOidcRefreshTokenStoreError {
    Database(rusqlite::Error),
    InvalidStoredState,
    InactiveClient,
    LockPoisoned,
}

impl fmt::Display for SqliteOidcRefreshTokenStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OIDC refresh-token storage failed")
    }
}

impl Error for SqliteOidcRefreshTokenStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl SqliteOidcRefreshTokenStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteOidcRefreshTokenStoreError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteOidcRefreshTokenStoreError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteOidcRefreshTokenStoreError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        oidc_schema::initialize(&connection).map_err(Self::database_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteOidcRefreshTokenStoreError {
        SqliteOidcRefreshTokenStoreError::Database(error)
    }
}

impl OidcRefreshTokenStore for SqliteOidcRefreshTokenStore {
    type Error = SqliteOidcRefreshTokenStoreError;

    fn insert_family(&self, family: &RefreshTokenFamily) -> Result<(), Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let inserted = connection.execute(
            "INSERT INTO oidc_token_family
             (id, client_id, installation_id, user_id, session_id, current_digest, generation, expires_at)
             SELECT ?1, client_id, installation_id, ?3, ?4, ?5, ?6, ?7
             FROM oidc_client WHERE client_id = ?2 AND revoked_at IS NULL",
            params![family.id(), family.client_id().as_str(), family.user_id().to_string(),
                family.session_id().to_string(), &family.current_digest()[..],
                i64::try_from(family.generation()).map_err(|_| Self::Error::InvalidStoredState)?,
                timestamp(family.expires_at())?],
        ).map_err(Self::database_error)?;
        if inserted != 1 {
            return Err(Self::Error::InactiveClient);
        }
        Ok(())
    }

    fn rotate(&self, raw: &str, now: UnixTimestamp) -> Result<RefreshRotation, Self::Error> {
        if raw.len() != 43 {
            return Ok(RefreshRotation::Invalid);
        }
        let digest = RefreshTokenFamily::digest_raw(raw);
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;
        let stored = transaction
            .query_row(
                "SELECT f.id, f.client_id, f.user_id, f.session_id, f.current_digest,
                    f.generation, f.expires_at, f.revoked_at
             FROM oidc_token_family f JOIN oidc_client c ON c.client_id = f.client_id
             WHERE f.current_digest = ?1 AND c.revoked_at IS NULL",
                [&digest[..]],
                read_family,
            )
            .optional()
            .map_err(Self::database_error)?;
        if let Some(stored) = stored {
            let mut family = stored.into_domain()?;
            let old_digest = *family.current_digest();
            let Ok(next) = family.rotate(raw, now) else {
                return Ok(RefreshRotation::Invalid);
            };
            transaction
                .execute(
                    "INSERT INTO oidc_refresh_spent (digest, family_id) VALUES (?1, ?2)",
                    params![&old_digest[..], family.id()],
                )
                .map_err(Self::database_error)?;
            transaction
                .execute(
                    "UPDATE oidc_token_family SET current_digest = ?2, generation = ?3
                 WHERE id = ?1 AND current_digest = ?4 AND revoked_at IS NULL",
                    params![
                        family.id(),
                        &family.current_digest()[..],
                        i64::try_from(family.generation())
                            .map_err(|_| Self::Error::InvalidStoredState)?,
                        &old_digest[..]
                    ],
                )
                .map_err(Self::database_error)?;
            transaction.commit().map_err(Self::database_error)?;
            return Ok(RefreshRotation::Rotated(next));
        }
        let spent_family: Option<String> = transaction
            .query_row(
                "SELECT family_id FROM oidc_refresh_spent WHERE digest = ?1",
                [&digest[..]],
                |row| row.get(0),
            )
            .optional()
            .map_err(Self::database_error)?;
        if let Some(family_id) = spent_family {
            transaction.execute(
                "UPDATE oidc_token_family SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
                params![family_id, timestamp(now)?],
            ).map_err(Self::database_error)?;
            transaction.execute(
                "UPDATE oidc_access_token SET revoked_at = ?2 WHERE family_id = ?1 AND revoked_at IS NULL",
                params![family_id, timestamp(now)?],
            ).map_err(Self::database_error)?;
            transaction.commit().map_err(Self::database_error)?;
            return Ok(RefreshRotation::ReplayRevoked);
        }
        Ok(RefreshRotation::Invalid)
    }

    fn revoke_for_session(
        &self,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<usize, Self::Error> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;
        let mut revoked = 0;
        for table in [
            "oidc_authorization_transaction",
            "oidc_authorization_code",
            "oidc_token_family",
            "oidc_access_token",
        ] {
            revoked += transaction.execute(
                &format!("UPDATE {table} SET revoked_at = ?2 WHERE session_id = ?1 AND revoked_at IS NULL"),
                params![session_id.to_string(), timestamp(now)?],
            ).map_err(Self::database_error)?;
        }
        transaction.commit().map_err(Self::database_error)?;
        Ok(revoked)
    }
}

struct StoredFamily {
    id: String,
    client_id: String,
    user_id: String,
    session_id: String,
    current_digest: Vec<u8>,
    generation: i64,
    expires_at: i64,
    revoked_at: Option<i64>,
}

impl StoredFamily {
    fn into_domain(self) -> Result<RefreshTokenFamily, SqliteOidcRefreshTokenStoreError> {
        RefreshTokenFamily::restore(
            self.id,
            UserId::parse(&self.user_id)
                .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)?,
            SessionId::parse(&self.session_id)
                .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)?,
            OidcClientId::parse(self.client_id)
                .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)?,
            self.current_digest
                .try_into()
                .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)?,
            u64::try_from(self.generation)
                .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)?,
            read_timestamp(self.expires_at)?,
            self.revoked_at.map(read_timestamp).transpose()?,
        )
        .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)
    }
}

fn read_family(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredFamily> {
    Ok(StoredFamily {
        id: row.get(0)?,
        client_id: row.get(1)?,
        user_id: row.get(2)?,
        session_id: row.get(3)?,
        current_digest: row.get(4)?,
        generation: row.get(5)?,
        expires_at: row.get(6)?,
        revoked_at: row.get(7)?,
    })
}

fn timestamp(value: UnixTimestamp) -> Result<i64, SqliteOidcRefreshTokenStoreError> {
    i64::try_from(value.as_seconds())
        .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)
}

fn read_timestamp(value: i64) -> Result<UnixTimestamp, SqliteOidcRefreshTokenStoreError> {
    u64::try_from(value)
        .map(UnixTimestamp::from_seconds)
        .map_err(|_| SqliteOidcRefreshTokenStoreError::InvalidStoredState)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{AppId, InstallationId, OidcClientType, OidcScope};
    use rumahl_oidc_provider::{OidcClientRecord, OidcClientRepository, OidcRedirectUri};

    use crate::SqliteOidcClientRepository;

    fn client() -> OidcClientRecord {
        OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Public,
            OidcRedirectUri::parse("https://cloud.rumahl.dev/oidc/callback").unwrap(),
            vec![OidcScope::OpenId, OidcScope::OfflineAccess],
            None,
            UnixTimestamp::from_seconds(100),
            None,
        )
        .unwrap()
    }

    #[test]
    fn refresh_rotation_and_replay_survive_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokens.sqlite3");
        let client = client();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let (family, first) = RefreshTokenFamily::create(
            UserId::new(),
            SessionId::new(),
            client.client_id().clone(),
            UnixTimestamp::from_seconds(300),
            UnixTimestamp::from_seconds(100),
        )
        .unwrap();
        SqliteOidcRefreshTokenStore::open(&path)
            .unwrap()
            .insert_family(&family)
            .unwrap();
        let store = SqliteOidcRefreshTokenStore::open(&path).unwrap();
        let RefreshRotation::Rotated(next) = store
            .rotate(&first, UnixTimestamp::from_seconds(110))
            .unwrap()
        else {
            panic!("first rotation must succeed");
        };
        drop(store);
        let store = SqliteOidcRefreshTokenStore::open(&path).unwrap();
        assert_eq!(
            store
                .rotate(&first, UnixTimestamp::from_seconds(111))
                .unwrap(),
            RefreshRotation::ReplayRevoked
        );
        assert_eq!(
            store
                .rotate(&next, UnixTimestamp::from_seconds(112))
                .unwrap(),
            RefreshRotation::Invalid
        );
    }

    #[test]
    fn session_logout_revokes_only_its_family() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokens.sqlite3");
        let client = client();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let first_session = SessionId::new();
        let second_session = SessionId::new();
        let (first, first_raw) = RefreshTokenFamily::create(
            UserId::new(),
            first_session,
            client.client_id().clone(),
            UnixTimestamp::from_seconds(300),
            UnixTimestamp::from_seconds(100),
        )
        .unwrap();
        let (second, second_raw) = RefreshTokenFamily::create(
            UserId::new(),
            second_session,
            client.client_id().clone(),
            UnixTimestamp::from_seconds(300),
            UnixTimestamp::from_seconds(100),
        )
        .unwrap();
        let store = SqliteOidcRefreshTokenStore::open(&path).unwrap();
        store.insert_family(&first).unwrap();
        store.insert_family(&second).unwrap();
        assert_eq!(
            store
                .revoke_for_session(first_session, UnixTimestamp::from_seconds(110))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .rotate(&first_raw, UnixTimestamp::from_seconds(111))
                .unwrap(),
            RefreshRotation::Invalid
        );
        assert!(matches!(
            store
                .rotate(&second_raw, UnixTimestamp::from_seconds(111))
                .unwrap(),
            RefreshRotation::Rotated(_)
        ));
    }
}
