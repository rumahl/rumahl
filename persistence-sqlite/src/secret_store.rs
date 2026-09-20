use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rumahl_core::{
    AppId, AppIdentity, InstallationId, InstallationIdError, PublisherId, PublisherIdError,
    SecretId, SecretIdError, SecretPurpose, SecretPurposeError, SecretRecord, SecretStore,
    SecretValue, SecretValueError, UnixTimestamp,
};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use zeroize::Zeroizing;

const KEY_LENGTH: usize = 32;
const NONCE_LENGTH: usize = 12;
const ASSOCIATED_DATA_VERSION: &[u8] = b"rumahl-secret-v1";

pub trait SecretEncryptionKeyProvider {
    type Error: Error + Send + Sync + 'static;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error>;

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretEncryptionKeyId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretEncryptionKeyIdError {
    Empty,
    TooLong,
    InvalidCharacter,
}

pub struct SecretEncryptionKey {
    id: SecretEncryptionKeyId,
    value: Zeroizing<[u8; KEY_LENGTH]>,
}

#[derive(Debug)]
pub struct SqliteSecretStore<K> {
    connection: Mutex<Connection>,
    key_provider: K,
}

#[derive(Debug)]
pub enum SqliteSecretStoreError<KeyError> {
    Database(rusqlite::Error),
    LockPoisoned,
    KeyProvider(KeyError),
    InvalidKeyLength,
    RandomGeneration(getrandom::Error),
    EncryptionFailed,
    AuthenticationFailed,
    AlreadyExists,
    InvalidSecretId(SecretIdError),
    InvalidInstallationId(InstallationIdError),
    InvalidAppId(String),
    InvalidPublisherId(PublisherIdError),
    InvalidPurpose(SecretPurposeError),
    InvalidValue(SecretValueError),
    InvalidKeyId(SecretEncryptionKeyIdError),
    InvalidNonceLength(usize),
    InvalidTimestamp(i64),
    MissingEncryptionKey(String),
    ValueOutsideSqliteRange { field: &'static str, value: u64 },
}

impl SecretEncryptionKeyId {
    pub const MAX_LENGTH: usize = 120;

    pub fn parse(value: impl Into<String>) -> Result<Self, SecretEncryptionKeyIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(SecretEncryptionKeyIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(SecretEncryptionKeyIdError::TooLong);
        }

        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(SecretEncryptionKeyIdError::InvalidCharacter);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretEncryptionKeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for SecretEncryptionKeyIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "secret encryption key ID cannot be empty"),
            Self::TooLong => write!(
                f,
                "secret encryption key ID cannot exceed {} bytes",
                SecretEncryptionKeyId::MAX_LENGTH
            ),
            Self::InvalidCharacter => {
                write!(f, "secret encryption key ID contains an invalid character")
            }
        }
    }
}

impl Error for SecretEncryptionKeyIdError {}

impl SecretEncryptionKey {
    pub fn new(id: SecretEncryptionKeyId, value: [u8; KEY_LENGTH]) -> Self {
        Self {
            id,
            value: Zeroizing::new(value),
        }
    }

    pub fn id(&self) -> &SecretEncryptionKeyId {
        &self.id
    }

    fn as_bytes(&self) -> &[u8; KEY_LENGTH] {
        &self.value
    }
}

impl fmt::Debug for SecretEncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretEncryptionKey")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl<K> SqliteSecretStore<K>
where
    K: SecretEncryptionKeyProvider,
{
    pub fn open(
        path: impl AsRef<Path>,
        key_provider: K,
    ) -> Result<Self, SqliteSecretStoreError<K::Error>> {
        Self::from_connection(
            Connection::open(path).map_err(SqliteSecretStoreError::Database)?,
            key_provider,
        )
    }

    pub fn open_in_memory(key_provider: K) -> Result<Self, SqliteSecretStoreError<K::Error>> {
        Self::from_connection(
            Connection::open_in_memory().map_err(SqliteSecretStoreError::Database)?,
            key_provider,
        )
    }

    fn from_connection(
        connection: Connection,
        key_provider: K,
    ) -> Result<Self, SqliteSecretStoreError<K::Error>> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(SqliteSecretStoreError::Database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS encrypted_app_secret (
                     secret_id TEXT PRIMARY KEY,
                     installation_id TEXT NOT NULL,
                     app_id TEXT NOT NULL,
                     publisher_id TEXT NOT NULL,
                     purpose TEXT NOT NULL,
                     key_id TEXT NOT NULL,
                     nonce BLOB NOT NULL CHECK (length(nonce) = 12),
                     ciphertext BLOB NOT NULL CHECK (length(ciphertext) >= 17),
                     created_at INTEGER NOT NULL CHECK (created_at >= 0),
                     UNIQUE (installation_id, purpose),
                     UNIQUE (key_id, nonce)
                 );

                 CREATE INDEX IF NOT EXISTS encrypted_app_secret_installation
                 ON encrypted_app_secret(installation_id);",
            )
            .map_err(SqliteSecretStoreError::Database)?;

        Ok(Self {
            connection: Mutex::new(connection),
            key_provider,
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, SqliteSecretStoreError<K::Error>> {
        self.connection
            .lock()
            .map_err(|_| SqliteSecretStoreError::LockPoisoned)
    }

    fn encrypt(
        &self,
        secret: &SecretRecord,
    ) -> Result<EncryptedSecret, SqliteSecretStoreError<K::Error>> {
        let key = self
            .key_provider
            .active_key()
            .map_err(SqliteSecretStoreError::KeyProvider)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|_| SqliteSecretStoreError::InvalidKeyLength)?;
        let mut nonce = [0_u8; NONCE_LENGTH];
        getrandom::fill(&mut nonce).map_err(SqliteSecretStoreError::RandomGeneration)?;
        let associated_data = associated_data(
            secret.id(),
            secret.owner(),
            secret.purpose(),
            key.id(),
            secret.created_at(),
        );
        let nonce_value = Nonce::try_from(&nonce[..])
            .map_err(|_| SqliteSecretStoreError::InvalidNonceLength(nonce.len()))?;
        let ciphertext = cipher
            .encrypt(
                &nonce_value,
                Payload {
                    msg: secret.value().as_bytes(),
                    aad: &associated_data,
                },
            )
            .map_err(|_| SqliteSecretStoreError::EncryptionFailed)?;

        Ok(EncryptedSecret {
            key_id: key.id,
            nonce,
            ciphertext,
        })
    }

    fn decode(
        &self,
        stored: StoredSecret,
    ) -> Result<SecretRecord, SqliteSecretStoreError<K::Error>> {
        let id =
            SecretId::parse(&stored.secret_id).map_err(SqliteSecretStoreError::InvalidSecretId)?;
        let installation_id = InstallationId::parse(&stored.installation_id)
            .map_err(SqliteSecretStoreError::InvalidInstallationId)?;
        let app_id = AppId::parse(&stored.app_id)
            .map_err(|error| SqliteSecretStoreError::InvalidAppId(error.to_string()))?;
        let publisher_id = PublisherId::parse(&stored.publisher_id)
            .map_err(SqliteSecretStoreError::InvalidPublisherId)?;
        let owner = AppIdentity::new(app_id, installation_id, publisher_id);
        let purpose =
            SecretPurpose::parse(stored.purpose).map_err(SqliteSecretStoreError::InvalidPurpose)?;
        let key_id = SecretEncryptionKeyId::parse(stored.key_id)
            .map_err(SqliteSecretStoreError::InvalidKeyId)?;
        let created_at = u64::try_from(stored.created_at)
            .map(UnixTimestamp::from_seconds)
            .map_err(|_| SqliteSecretStoreError::InvalidTimestamp(stored.created_at))?;

        if stored.nonce.len() != NONCE_LENGTH {
            return Err(SqliteSecretStoreError::InvalidNonceLength(
                stored.nonce.len(),
            ));
        }

        let key = self
            .key_provider
            .key_by_id(&key_id)
            .map_err(SqliteSecretStoreError::KeyProvider)?
            .ok_or_else(|| SqliteSecretStoreError::MissingEncryptionKey(key_id.to_string()))?;
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|_| SqliteSecretStoreError::InvalidKeyLength)?;
        let associated_data = associated_data(&id, &owner, &purpose, &key_id, created_at);
        let nonce = Nonce::try_from(stored.nonce.as_slice())
            .map_err(|_| SqliteSecretStoreError::InvalidNonceLength(stored.nonce.len()))?;
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &stored.ciphertext,
                    aad: &associated_data,
                },
            )
            .map_err(|_| SqliteSecretStoreError::AuthenticationFailed)?;
        let value = SecretValue::new(plaintext).map_err(SqliteSecretStoreError::InvalidValue)?;

        Ok(SecretRecord::restore(id, owner, purpose, value, created_at))
    }
}

impl<K> SecretStore for SqliteSecretStore<K>
where
    K: SecretEncryptionKeyProvider,
{
    type Error = SqliteSecretStoreError<K::Error>;

    fn insert(&self, secret: &SecretRecord) -> Result<(), Self::Error> {
        let encrypted = self.encrypt(secret)?;
        let created_at = i64::try_from(secret.created_at().as_seconds()).map_err(|_| {
            SqliteSecretStoreError::ValueOutsideSqliteRange {
                field: "created_at",
                value: secret.created_at().as_seconds(),
            }
        })?;
        let connection = self.connection()?;
        let inserted = connection.execute(
            "INSERT INTO encrypted_app_secret (
                 secret_id, installation_id, app_id, publisher_id, purpose, key_id,
                 nonce, ciphertext, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                secret.id().to_string(),
                secret.owner().installation_id().to_string(),
                secret.owner().app_id().as_str(),
                secret.owner().publisher_id().as_str(),
                secret.purpose().as_str(),
                encrypted.key_id.as_str(),
                &encrypted.nonce[..],
                encrypted.ciphertext,
                created_at,
            ],
        );

        match inserted {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation =>
            {
                Err(SqliteSecretStoreError::AlreadyExists)
            }
            Err(error) => Err(SqliteSecretStoreError::Database(error)),
        }
    }

    fn find_by_owner_and_purpose(
        &self,
        owner: &AppIdentity,
        purpose: &SecretPurpose,
    ) -> Result<Option<SecretRecord>, Self::Error> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                "SELECT secret_id, installation_id, app_id, publisher_id, purpose, key_id,
                        nonce, ciphertext, created_at
                 FROM encrypted_app_secret
                 WHERE installation_id = ?1 AND purpose = ?2",
                params![owner.installation_id().to_string(), purpose.as_str()],
                |row| {
                    Ok(StoredSecret {
                        secret_id: row.get(0)?,
                        installation_id: row.get(1)?,
                        app_id: row.get(2)?,
                        publisher_id: row.get(3)?,
                        purpose: row.get(4)?,
                        key_id: row.get(5)?,
                        nonce: row.get(6)?,
                        ciphertext: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(SqliteSecretStoreError::Database)?;

        let Some(stored) = stored else {
            return Ok(None);
        };
        let record = self.decode(stored)?;

        if record.owner() != owner || record.purpose() != purpose {
            return Err(SqliteSecretStoreError::AuthenticationFailed);
        }

        Ok(Some(record))
    }

    fn remove_for_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<usize, Self::Error> {
        self.connection()?
            .execute(
                "DELETE FROM encrypted_app_secret WHERE installation_id = ?1",
                [installation_id.to_string()],
            )
            .map_err(SqliteSecretStoreError::Database)
    }
}

struct StoredSecret {
    secret_id: String,
    installation_id: String,
    app_id: String,
    publisher_id: String,
    purpose: String,
    key_id: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    created_at: i64,
}

struct EncryptedSecret {
    key_id: SecretEncryptionKeyId,
    nonce: [u8; NONCE_LENGTH],
    ciphertext: Vec<u8>,
}

fn associated_data(
    id: &SecretId,
    owner: &AppIdentity,
    purpose: &SecretPurpose,
    key_id: &SecretEncryptionKeyId,
    created_at: UnixTimestamp,
) -> Vec<u8> {
    let mut data = Vec::new();
    append_field(&mut data, ASSOCIATED_DATA_VERSION);
    append_field(&mut data, id.to_string().as_bytes());
    append_field(&mut data, owner.installation_id().to_string().as_bytes());
    append_field(&mut data, owner.app_id().as_str().as_bytes());
    append_field(&mut data, owner.publisher_id().as_str().as_bytes());
    append_field(&mut data, purpose.as_str().as_bytes());
    append_field(&mut data, key_id.as_str().as_bytes());
    append_field(&mut data, &created_at.as_seconds().to_be_bytes());
    data
}

fn append_field(target: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("secret metadata length must fit u64");
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(value);
}

impl<KeyError> fmt::Display for SqliteSecretStoreError<KeyError>
where
    KeyError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite secret store failed: {error}"),
            Self::LockPoisoned => write!(f, "secret store lock is poisoned"),
            Self::KeyProvider(_) => write!(f, "secret encryption key provider failed"),
            Self::InvalidKeyLength => write!(f, "secret encryption key length is invalid"),
            Self::RandomGeneration(_) => write!(f, "secret nonce generation failed"),
            Self::EncryptionFailed => write!(f, "secret encryption failed"),
            Self::AuthenticationFailed => write!(f, "encrypted secret authentication failed"),
            Self::AlreadyExists => write!(f, "secret already exists for this installation purpose"),
            Self::InvalidSecretId(error) => write!(f, "stored secret ID is invalid: {error}"),
            Self::InvalidInstallationId(error) => {
                write!(f, "stored secret installation ID is invalid: {error}")
            }
            Self::InvalidAppId(message) => write!(f, "stored secret app ID is invalid: {message}"),
            Self::InvalidPublisherId(error) => {
                write!(f, "stored secret publisher ID is invalid: {error}")
            }
            Self::InvalidPurpose(error) => write!(f, "stored secret purpose is invalid: {error}"),
            Self::InvalidValue(error) => write!(f, "stored secret value is invalid: {error}"),
            Self::InvalidKeyId(error) => write!(f, "stored secret key ID is invalid: {error}"),
            Self::InvalidNonceLength(length) => {
                write!(f, "stored secret nonce length {length} is invalid")
            }
            Self::InvalidTimestamp(value) => {
                write!(f, "stored secret timestamp {value} is invalid")
            }
            Self::MissingEncryptionKey(key_id) => {
                write!(f, "secret encryption key {key_id:?} is unavailable")
            }
            Self::ValueOutsideSqliteRange { field, value } => {
                write!(f, "secret {field} {value} is outside SQLite range")
            }
        }
    }
}

impl<KeyError> Error for SqliteSecretStoreError<KeyError>
where
    KeyError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::KeyProvider(error) => Some(error),
            Self::RandomGeneration(error) => Some(error),
            Self::InvalidSecretId(error) => Some(error),
            Self::InvalidInstallationId(error) => Some(error),
            Self::InvalidPublisherId(error) => Some(error),
            Self::InvalidPurpose(error) => Some(error),
            Self::InvalidValue(error) => Some(error),
            Self::InvalidKeyId(error) => Some(error),
            Self::LockPoisoned
            | Self::InvalidKeyLength
            | Self::EncryptionFailed
            | Self::AuthenticationFailed
            | Self::AlreadyExists
            | Self::InvalidAppId(_)
            | Self::InvalidNonceLength(_)
            | Self::InvalidTimestamp(_)
            | Self::MissingEncryptionKey(_)
            | Self::ValueOutsideSqliteRange { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const KEY_BYTES: [u8; KEY_LENGTH] = [0x42; KEY_LENGTH];

    struct TestKeyProvider;

    impl SecretEncryptionKeyProvider for TestKeyProvider {
        type Error = Infallible;

        fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
            Ok(test_key())
        }

        fn key_by_id(
            &self,
            id: &SecretEncryptionKeyId,
        ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
            Ok((id.as_str() == "device-key-1").then(test_key))
        }
    }

    struct MissingKeyProvider;

    impl SecretEncryptionKeyProvider for MissingKeyProvider {
        type Error = Infallible;

        fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
            Ok(test_key())
        }

        fn key_by_id(
            &self,
            _id: &SecretEncryptionKeyId,
        ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
            Ok(None)
        }
    }

    fn test_key() -> SecretEncryptionKey {
        SecretEncryptionKey::new(
            SecretEncryptionKeyId::parse("device-key-1").unwrap(),
            KEY_BYTES,
        )
    }

    fn owner() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn purpose() -> SecretPurpose {
        SecretPurpose::parse("rumahl.oidc.client-secret").unwrap()
    }

    fn record(owner: AppIdentity) -> SecretRecord {
        SecretRecord::new(
            owner,
            purpose(),
            SecretValue::new(b"super-secret-value".to_vec()).unwrap(),
            UnixTimestamp::from_seconds(100),
        )
    }

    fn database_path(test_name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rumahl-{test_name}-{nonce}.sqlite3"))
    }

    #[test]
    fn encrypted_secret_survives_reopen_without_plaintext_at_rest() {
        let path = database_path("secret-store");
        let owner = owner();
        let secret = record(owner.clone());

        {
            let store = SqliteSecretStore::open(&path, TestKeyProvider).unwrap();
            store.insert(&secret).unwrap();
            let connection = store.connection().unwrap();
            let ciphertext: Vec<u8> = connection
                .query_row("SELECT ciphertext FROM encrypted_app_secret", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(
                !ciphertext
                    .windows(b"super-secret-value".len())
                    .any(|window| window == b"super-secret-value")
            );
        }

        let store = SqliteSecretStore::open(&path, TestKeyProvider).unwrap();
        let restored = store
            .find_by_owner_and_purpose(&owner, &purpose())
            .unwrap()
            .unwrap();
        assert_eq!(restored.value().as_bytes(), b"super-secret-value");

        drop(store);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn metadata_tampering_fails_authentication() {
        let store = SqliteSecretStore::open_in_memory(TestKeyProvider).unwrap();
        let owner = owner();
        store.insert(&record(owner.clone())).unwrap();
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE encrypted_app_secret SET purpose = 'rumahl.oidc.other-secret'",
                [],
            )
            .unwrap();

        assert!(matches!(
            store.find_by_owner_and_purpose(
                &owner,
                &SecretPurpose::parse("rumahl.oidc.other-secret").unwrap(),
            ),
            Err(SqliteSecretStoreError::AuthenticationFailed)
        ));
    }

    #[test]
    fn missing_root_key_does_not_return_ciphertext_as_plaintext() {
        let path = database_path("missing-secret-key");
        let owner = owner();
        {
            let store = SqliteSecretStore::open(&path, TestKeyProvider).unwrap();
            store.insert(&record(owner.clone())).unwrap();
        }

        let store = SqliteSecretStore::open(&path, MissingKeyProvider).unwrap();
        assert!(matches!(
            store.find_by_owner_and_purpose(&owner, &purpose()),
            Err(SqliteSecretStoreError::MissingEncryptionKey(_))
        ));

        drop(store);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn enforces_one_secret_per_installation_purpose_and_removes_atomically() {
        let store = SqliteSecretStore::open_in_memory(TestKeyProvider).unwrap();
        let owner = owner();
        store.insert(&record(owner.clone())).unwrap();

        assert!(matches!(
            store.insert(&record(owner.clone())),
            Err(SqliteSecretStoreError::AlreadyExists)
        ));
        assert_eq!(
            store
                .remove_for_installation(owner.installation_id())
                .unwrap(),
            1
        );
        assert!(
            store
                .find_by_owner_and_purpose(&owner, &purpose())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn key_debug_output_is_redacted() {
        let key = test_key();

        assert!(!format!("{key:?}").contains("42424242"));
    }
}
