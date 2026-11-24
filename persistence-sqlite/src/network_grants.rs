//! Durable user consent for app network origins.
//!
//! An app may only reach an origin the user explicitly approved for that
//! installation. This store holds the durable half of that consent; the live
//! service keeps session-only approvals in memory so a restart grants nothing.
//!
//! Origins are exact `scheme://host[:port]` strings with no path, credentials
//! or fragment. The service validates the shape before writing, and every
//! request re-checks the approval against the live installation and user.

use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredNetworkGrant {
    pub origin: String,
    /// Unix seconds; `None` means until revoked.
    pub expires_at: Option<u64>,
    /// The user explicitly allowed private and loopback destinations.
    pub allow_private: bool,
}

#[derive(Debug)]
pub enum NetworkGrantStoreError {
    Database(rusqlite::Error),
    LockPoisoned,
    InvalidTimestamp(i64),
}

pub struct SqliteNetworkGrants {
    connection: Mutex<Connection>,
}

impl fmt::Debug for SqliteNetworkGrants {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteNetworkGrants")
            .finish_non_exhaustive()
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

impl SqliteNetworkGrants {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NetworkGrantStoreError> {
        Self::from_connection(Connection::open(path).map_err(NetworkGrantStoreError::Database)?)
    }

    pub fn open_in_memory() -> Result<Self, NetworkGrantStoreError> {
        Self::from_connection(
            Connection::open_in_memory().map_err(NetworkGrantStoreError::Database)?,
        )
    }

    fn from_connection(connection: Connection) -> Result<Self, NetworkGrantStoreError> {
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(NetworkGrantStoreError::Database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS app_network_grant (
                     installation_id TEXT NOT NULL,
                     user_id TEXT NOT NULL,
                     origin TEXT NOT NULL,
                     created_at INTEGER NOT NULL CHECK (created_at >= 0),
                     expires_at INTEGER,
                     allow_private INTEGER NOT NULL DEFAULT 0 CHECK (allow_private IN (0, 1)),
                     PRIMARY KEY (installation_id, user_id, origin)
                 );
                 CREATE INDEX IF NOT EXISTS app_network_grant_installation
                 ON app_network_grant(installation_id);",
            )
            .map_err(NetworkGrantStoreError::Database)?;
        // Databases created before private-network scopes get the column too.
        let columns: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA table_info(app_network_grant)")
                .map_err(NetworkGrantStoreError::Database)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(NetworkGrantStoreError::Database)?;
            rows.collect::<Result<_, _>>()
                .map_err(NetworkGrantStoreError::Database)?
        };
        if !columns.iter().any(|column| column == "allow_private") {
            connection
                .execute(
                    "ALTER TABLE app_network_grant
                     ADD COLUMN allow_private INTEGER NOT NULL DEFAULT 0
                     CHECK (allow_private IN (0, 1))",
                    [],
                )
                .map_err(NetworkGrantStoreError::Database)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, NetworkGrantStoreError> {
        self.connection
            .lock()
            .map_err(|_| NetworkGrantStoreError::LockPoisoned)
    }

    /// Adds or replaces one durable approval. An expired or missing timestamp
    /// clears the row instead: an inactive consent is not a grant.
    pub fn grant(
        &self,
        installation: &str,
        user_id: &str,
        origin: &str,
        expires_at: Option<u64>,
        allow_private: bool,
    ) -> Result<(), NetworkGrantStoreError> {
        if expires_at.is_some_and(|expires| expires <= now_seconds()) {
            return self.revoke(installation, user_id, origin);
        }
        let created_at = i64::try_from(now_seconds())
            .map_err(|_| NetworkGrantStoreError::InvalidTimestamp(i64::MAX))?;
        let expires_at = expires_at
            .map(i64::try_from)
            .transpose()
            .map_err(|_| NetworkGrantStoreError::InvalidTimestamp(i64::MAX))?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO app_network_grant (
                     installation_id, user_id, origin, created_at, expires_at, allow_private
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(installation_id, user_id, origin) DO UPDATE SET
                     created_at = excluded.created_at,
                     expires_at = excluded.expires_at,
                     allow_private = excluded.allow_private",
                params![
                    installation,
                    user_id,
                    origin,
                    created_at,
                    expires_at,
                    allow_private
                ],
            )
            .map_err(NetworkGrantStoreError::Database)?;
        Ok(())
    }

    pub fn revoke(
        &self,
        installation: &str,
        user_id: &str,
        origin: &str,
    ) -> Result<(), NetworkGrantStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM app_network_grant
                 WHERE installation_id = ?1 AND user_id = ?2 AND origin = ?3",
                params![installation, user_id, origin],
            )
            .map_err(NetworkGrantStoreError::Database)?;
        Ok(())
    }

    /// Every still-valid durable approval for one installation and user.
    pub fn list(
        &self,
        installation: &str,
        user_id: &str,
    ) -> Result<Vec<StoredNetworkGrant>, NetworkGrantStoreError> {
        let now = i64::try_from(now_seconds())
            .map_err(|_| NetworkGrantStoreError::InvalidTimestamp(i64::MAX))?;
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT origin, expires_at, allow_private FROM app_network_grant
                 WHERE installation_id = ?1 AND user_id = ?2
                 AND (expires_at IS NULL OR expires_at > ?3)
                 ORDER BY origin",
            )
            .map_err(NetworkGrantStoreError::Database)?;
        let rows = statement
            .query_map(params![installation, user_id, now], |row| {
                let expires_at: Option<i64> = row.get(1)?;
                let expires_at = expires_at
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, i64::MAX))?;
                let allow_private: i64 = row.get(2)?;
                Ok(StoredNetworkGrant {
                    origin: row.get(0)?,
                    expires_at,
                    allow_private: allow_private != 0,
                })
            })
            .map_err(NetworkGrantStoreError::Database)?;
        let mut grants = Vec::new();
        for row in rows {
            grants.push(row.map_err(NetworkGrantStoreError::Database)?);
        }
        Ok(grants)
    }

    pub fn is_granted(
        &self,
        installation: &str,
        user_id: &str,
        origin: &str,
    ) -> Result<bool, NetworkGrantStoreError> {
        self.grant_scope(installation, user_id, origin)
            .map(|scope| scope.is_some())
    }

    /// The stored private-network decision, or `None` when not granted.
    pub fn grant_scope(
        &self,
        installation: &str,
        user_id: &str,
        origin: &str,
    ) -> Result<Option<bool>, NetworkGrantStoreError> {
        let now = i64::try_from(now_seconds())
            .map_err(|_| NetworkGrantStoreError::InvalidTimestamp(i64::MAX))?;
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT allow_private FROM app_network_grant
                 WHERE installation_id = ?1 AND user_id = ?2 AND origin = ?3
                 AND (expires_at IS NULL OR expires_at > ?4)",
                params![installation, user_id, origin, now],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|scope| scope.map(|value| value != 0))
            .map_err(NetworkGrantStoreError::Database)
    }

    pub fn remove_for_installation(
        &self,
        installation: &str,
    ) -> Result<usize, NetworkGrantStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM app_network_grant WHERE installation_id = ?1",
                params![installation],
            )
            .map_err(NetworkGrantStoreError::Database)
    }
}

impl fmt::Display for NetworkGrantStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "network grant database failed: {error}"),
            Self::LockPoisoned => write!(formatter, "network grant store lock is poisoned"),
            Self::InvalidTimestamp(value) => {
                write!(formatter, "network grant timestamp {value} is invalid")
            }
        }
    }
}

impl Error for NetworkGrantStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::LockPoisoned | Self::InvalidTimestamp(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_grants_are_scoped_to_installation_and_user() {
        let store = SqliteNetworkGrants::open_in_memory().unwrap();
        store
            .grant(
                "installation",
                "user-a",
                "http://ha.local:8123",
                None,
                false,
            )
            .unwrap();
        store
            .grant("installation", "user-b", "http://other.local", None, false)
            .unwrap();

        assert!(
            store
                .is_granted("installation", "user-a", "http://ha.local:8123")
                .unwrap()
        );
        assert!(
            !store
                .is_granted("installation", "user-b", "http://ha.local:8123")
                .unwrap()
        );
        assert!(
            !store
                .is_granted("other", "user-a", "http://ha.local:8123")
                .unwrap()
        );
        assert_eq!(store.list("installation", "user-a").unwrap().len(), 1);
    }

    #[test]
    fn private_scope_round_trips_and_updates() {
        let store = SqliteNetworkGrants::open_in_memory().unwrap();
        store
            .grant("installation", "user", "http://ha.local", None, true)
            .unwrap();
        assert_eq!(
            store
                .grant_scope("installation", "user", "http://ha.local")
                .unwrap(),
            Some(true)
        );
        assert!(store.list("installation", "user").unwrap()[0].allow_private);
        store
            .grant("installation", "user", "http://ha.local", None, false)
            .unwrap();
        assert_eq!(
            store
                .grant_scope("installation", "user", "http://ha.local")
                .unwrap(),
            Some(false)
        );
    }

    #[test]
    fn expired_and_revoked_grants_do_not_authorize() {
        let store = SqliteNetworkGrants::open_in_memory().unwrap();
        store
            .grant("installation", "user", "http://ha.local", Some(1), false)
            .unwrap();
        assert!(store.list("installation", "user").unwrap().is_empty());
        assert!(
            !store
                .is_granted("installation", "user", "http://ha.local")
                .unwrap()
        );

        store
            .grant("installation", "user", "http://ha.local", None, false)
            .unwrap();
        assert!(
            store
                .is_granted("installation", "user", "http://ha.local")
                .unwrap()
        );
        store
            .revoke("installation", "user", "http://ha.local")
            .unwrap();
        assert!(
            !store
                .is_granted("installation", "user", "http://ha.local")
                .unwrap()
        );
    }

    #[test]
    fn uninstall_removes_all_grants_of_an_installation() {
        let store = SqliteNetworkGrants::open_in_memory().unwrap();
        store
            .grant("installation", "a", "http://a.local", None, false)
            .unwrap();
        store
            .grant("installation", "b", "http://b.local", None, false)
            .unwrap();
        assert_eq!(store.remove_for_installation("installation").unwrap(), 2);
        assert!(store.list("installation", "a").unwrap().is_empty());
    }
}
