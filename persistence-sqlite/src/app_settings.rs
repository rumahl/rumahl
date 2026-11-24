//! Persisted values for manifest-declared app settings.
//!
//! The signed manifest declares the shape of a setting; this store keeps the
//! value the user (or the app on the user's behalf) entered. Plain values are
//! JSON text. Secret values are sealed with the same field cipher used for
//! other sensitive columns and are never returned to a browser: callers can
//! only ask whether a secret is configured, or consume it server-side (for
//! example to attach it to a brokered network request).
//!
//! The encryption context binds a secret to its installation and key, so a
//! ciphertext moved to another setting or another installation fails
//! authentication instead of decrypting.

use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use zeroize::Zeroizing;

use crate::{FieldCipherError, FieldEnvelope, FieldKeyProvider, open_field, seal_field};

const CONTEXT_PREFIX: &str = "app-setting";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// One setting value as stored for an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAppSetting {
    pub key: String,
    /// JSON text for plain settings; `None` for secrets.
    pub value_text: Option<String>,
    pub secret: bool,
    pub updated_at: u64,
}

/// A value to write for one setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSettingWrite {
    /// JSON text, already validated against the manifest type.
    Plain(String),
    /// Secret bytes; sealed at rest and never readable through the list API.
    Secret(Vec<u8>),
}

#[derive(Debug)]
pub enum AppSettingStoreError {
    Database(rusqlite::Error),
    LockPoisoned,
    Cipher(FieldCipherError),
    InvalidTimestamp(i64),
    InvalidSealed(&'static str),
}

pub struct SqliteAppSettings {
    connection: Mutex<Connection>,
    provider: Arc<dyn FieldKeyProvider>,
}

impl fmt::Debug for SqliteAppSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteAppSettings")
            .finish_non_exhaustive()
    }
}

fn context(installation: &str, key: &str) -> Vec<u8> {
    format!("{CONTEXT_PREFIX}:{installation}:{key}").into_bytes()
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

impl SqliteAppSettings {
    pub fn open(
        path: impl AsRef<Path>,
        provider: Arc<dyn FieldKeyProvider>,
    ) -> Result<Self, AppSettingStoreError> {
        Self::from_connection(
            Connection::open(path).map_err(AppSettingStoreError::Database)?,
            provider,
        )
    }

    pub fn open_in_memory(
        provider: Arc<dyn FieldKeyProvider>,
    ) -> Result<Self, AppSettingStoreError> {
        Self::from_connection(
            Connection::open_in_memory().map_err(AppSettingStoreError::Database)?,
            provider,
        )
    }

    fn from_connection(
        connection: Connection,
        provider: Arc<dyn FieldKeyProvider>,
    ) -> Result<Self, AppSettingStoreError> {
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(AppSettingStoreError::Database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS app_setting (
                     installation_id TEXT NOT NULL,
                     key TEXT NOT NULL,
                     is_secret INTEGER NOT NULL CHECK (is_secret IN (0, 1)),
                     value_text TEXT,
                     sealed BLOB,
                     updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
                     PRIMARY KEY (installation_id, key),
                     CHECK (
                         (is_secret = 0 AND value_text IS NOT NULL AND sealed IS NULL)
                         OR (is_secret = 1 AND sealed IS NOT NULL AND value_text IS NULL)
                     )
                 );",
            )
            .map_err(AppSettingStoreError::Database)?;
        Ok(Self {
            connection: Mutex::new(connection),
            provider,
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppSettingStoreError> {
        self.connection
            .lock()
            .map_err(|_| AppSettingStoreError::LockPoisoned)
    }

    /// Every stored setting of one installation, secrets included as metadata.
    pub fn load(&self, installation: &str) -> Result<Vec<StoredAppSetting>, AppSettingStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT key, is_secret, value_text, updated_at
                 FROM app_setting WHERE installation_id = ?1 ORDER BY key",
            )
            .map_err(AppSettingStoreError::Database)?;
        let rows = statement
            .query_map(params![installation], |row| {
                let updated_at: i64 = row.get(3)?;
                Ok(StoredAppSetting {
                    key: row.get(0)?,
                    secret: row.get::<_, i64>(1)? != 0,
                    value_text: row.get(2)?,
                    updated_at: u64::try_from(updated_at)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, updated_at))?,
                })
            })
            .map_err(AppSettingStoreError::Database)?;
        let mut settings = Vec::new();
        for row in rows {
            settings.push(row.map_err(AppSettingStoreError::Database)?);
        }
        Ok(settings)
    }

    /// Writes one setting, replacing only that key.
    pub fn set(
        &self,
        installation: &str,
        key: &str,
        value: AppSettingWrite,
    ) -> Result<(), AppSettingStoreError> {
        let (secret, value_text, sealed) = match value {
            AppSettingWrite::Plain(text) => (false, Some(text), None),
            AppSettingWrite::Secret(bytes) => {
                // An empty secret is an explicit clear, not a configured empty
                // token. Keeping the "configured" flag honest is important:
                // the settings view and the network broker both rely on it.
                if bytes.is_empty() {
                    return self.remove(installation, key);
                }
                let envelope =
                    seal_field(self.provider.as_ref(), &context(installation, key), &bytes)
                        .map_err(AppSettingStoreError::Cipher)?;
                (true, None, Some(envelope.encode()))
            }
        };
        let updated_at = i64::try_from(now_seconds())
            .map_err(|_| AppSettingStoreError::InvalidTimestamp(i64::MAX))?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO app_setting (
                     installation_id, key, is_secret, value_text, sealed, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(installation_id, key) DO UPDATE SET
                     is_secret = excluded.is_secret,
                     value_text = excluded.value_text,
                     sealed = excluded.sealed,
                     updated_at = excluded.updated_at",
                params![
                    installation,
                    key,
                    i64::from(secret),
                    value_text,
                    sealed,
                    updated_at
                ],
            )
            .map_err(AppSettingStoreError::Database)?;
        Ok(())
    }

    /// Removes one setting. Used when a value is cleared.
    pub fn remove(&self, installation: &str, key: &str) -> Result<(), AppSettingStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM app_setting WHERE installation_id = ?1 AND key = ?2",
                params![installation, key],
            )
            .map_err(AppSettingStoreError::Database)?;
        Ok(())
    }

    /// Decrypts one secret for a server-side consumer. Returns `None` when the
    /// key is absent or is not a secret; authentication failures are errors.
    pub fn secret(
        &self,
        installation: &str,
        key: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, AppSettingStoreError> {
        let connection = self.connection()?;
        let sealed: Option<Vec<u8>> = connection
            .query_row(
                "SELECT sealed FROM app_setting
                 WHERE installation_id = ?1 AND key = ?2 AND is_secret = 1",
                params![installation, key],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppSettingStoreError::Database)?;
        let Some(sealed) = sealed else {
            return Ok(None);
        };
        let envelope = FieldEnvelope::decode(&sealed)
            .map_err(|_| AppSettingStoreError::InvalidSealed("envelope"))?;
        open_field(
            self.provider.as_ref(),
            &context(installation, key),
            &envelope,
        )
        .map(Some)
        .map_err(AppSettingStoreError::Cipher)
    }

    /// Drops every value of an installation. Uninstall calls this after the
    /// runtime and grants are gone, so a reinstall never inherits old values.
    pub fn remove_for_installation(
        &self,
        installation: &str,
    ) -> Result<usize, AppSettingStoreError> {
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM app_setting WHERE installation_id = ?1",
                params![installation],
            )
            .map_err(AppSettingStoreError::Database)
    }
}

impl fmt::Display for AppSettingStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "app setting database failed: {error}"),
            Self::LockPoisoned => write!(formatter, "app setting store lock is poisoned"),
            Self::Cipher(error) => write!(formatter, "app setting encryption failed: {error}"),
            Self::InvalidTimestamp(value) => {
                write!(formatter, "app setting timestamp {value} is invalid")
            }
            Self::InvalidSealed(field) => {
                write!(formatter, "app setting {field} is malformed")
            }
        }
    }
}

impl Error for AppSettingStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Cipher(error) => Some(error),
            Self::LockPoisoned | Self::InvalidTimestamp(_) | Self::InvalidSealed(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::{
        SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    };

    struct Key;

    impl SecretEncryptionKeyProvider for Key {
        type Error = std::convert::Infallible;

        fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
            Ok(SecretEncryptionKey::new(
                SecretEncryptionKeyId::parse("test-key").unwrap(),
                [7_u8; 32],
            ))
        }

        fn key_by_id(
            &self,
            id: &SecretEncryptionKeyId,
        ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
            Ok((id.as_str() == "test-key").then(|| {
                SecretEncryptionKey::new(
                    SecretEncryptionKeyId::parse("test-key").unwrap(),
                    [7_u8; 32],
                )
            }))
        }
    }

    fn store() -> SqliteAppSettings {
        SqliteAppSettings::open_in_memory(Arc::new(Key)).unwrap()
    }

    #[test]
    fn round_trips_plain_and_secret_values_without_exposing_the_secret() {
        let store = store();
        store
            .set(
                "installation",
                "url",
                AppSettingWrite::Plain("\"https://ha.local\"".into()),
            )
            .unwrap();
        store
            .set(
                "installation",
                "token",
                AppSettingWrite::Secret(b"super-secret".to_vec()),
            )
            .unwrap();

        let values = store.load("installation").unwrap();
        assert_eq!(values.len(), 2);
        let token = values.iter().find(|value| value.key == "token").unwrap();
        assert!(token.secret);
        assert_eq!(token.value_text, None);

        let secret = store.secret("installation", "token").unwrap().unwrap();
        assert_eq!(&secret[..], b"super-secret");
        assert!(store.secret("installation", "url").unwrap().is_none());
    }

    #[test]
    fn a_secret_cannot_be_moved_to_another_key_or_installation() {
        let store = store();
        store
            .set(
                "installation",
                "token",
                AppSettingWrite::Secret(b"secret".to_vec()),
            )
            .unwrap();
        store
            .set("other", "token", AppSettingWrite::Secret(b"other".to_vec()))
            .unwrap();

        assert_eq!(
            &store.secret("other", "token").unwrap().unwrap()[..],
            b"other"
        );
        // Replacing the context by renaming the row is not possible through
        // the API; the ciphertext itself is bound to key + installation.
        let connection = store.connection().unwrap();
        connection
            .execute(
                "UPDATE app_setting SET key = 'moved' WHERE installation_id = 'installation'",
                [],
            )
            .unwrap();
        drop(connection);
        assert!(store.secret("installation", "moved").is_err());
    }

    #[test]
    fn empty_secret_clears_and_removes_the_key() {
        let store = store();
        store
            .set(
                "installation",
                "token",
                AppSettingWrite::Secret(b"secret".to_vec()),
            )
            .unwrap();
        store
            .set("installation", "token", AppSettingWrite::Secret(Vec::new()))
            .unwrap();
        assert!(store.load("installation").unwrap().is_empty());
        assert!(store.secret("installation", "token").unwrap().is_none());
    }

    #[test]
    fn uninstall_removes_every_value() {
        let store = store();
        store
            .set("installation", "a", AppSettingWrite::Plain("true".into()))
            .unwrap();
        store
            .set("installation", "b", AppSettingWrite::Plain("1".into()))
            .unwrap();
        assert_eq!(store.remove_for_installation("installation").unwrap(), 2);
        assert!(store.load("installation").unwrap().is_empty());
    }
}
