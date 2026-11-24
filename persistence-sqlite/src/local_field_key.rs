//! Local software key source for field-encrypted columns.
//!
//! Deployments with a TPM use the sealed-key provider. Development and
//! installations without TPM still need encrypted secrets (an app's access
//! token must never be stored as plaintext), so the platform can keep a random
//! 256-bit key in a `0600` file under its private state directory. The file is
//! created once and never enters the database, logs or a browser bundle.
//!
//! This is deliberately not a replacement for TPM sealing: the key sits on the
//! same disk as the ciphertext. It is a strict improvement over plaintext and
//! keeps behaviour identical across adapters.

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::secret_store::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyIdError,
    SecretEncryptionKeyProvider,
};

const KEY_LENGTH: usize = 32;
const KEY_ID: &str = "local-field-v1";

#[derive(Debug)]
pub enum LocalFieldKeyError {
    Io(std::io::Error),
    InvalidKeyLength(usize),
    Permissions,
    KeyId(SecretEncryptionKeyIdError),
    Random(getrandom::Error),
}

pub struct LocalFieldKeyProvider {
    id: SecretEncryptionKeyId,
    key: Zeroizing<[u8; KEY_LENGTH]>,
    path: PathBuf,
}

impl fmt::Debug for LocalFieldKeyProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalFieldKeyProvider")
            .field("path", &self.path)
            .field("key_id", &self.id)
            .finish_non_exhaustive()
    }
}

impl LocalFieldKeyProvider {
    /// Opens an existing key or creates a new one. The parent directory must
    /// already exist and be private to the platform account.
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, LocalFieldKeyError> {
        let path = path.as_ref().to_path_buf();
        let id = SecretEncryptionKeyId::parse(KEY_ID).map_err(LocalFieldKeyError::KeyId)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut bytes = [0_u8; KEY_LENGTH];
                getrandom::fill(&mut bytes).map_err(LocalFieldKeyError::Random)?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true).mode(0o600);
                let mut file = options.open(&path).map_err(LocalFieldKeyError::Io)?;
                file.write_all(&bytes).map_err(LocalFieldKeyError::Io)?;
                file.sync_all().map_err(LocalFieldKeyError::Io)?;
                bytes.to_vec()
            }
            Err(error) => return Err(LocalFieldKeyError::Io(error)),
        };
        if bytes.len() != KEY_LENGTH {
            return Err(LocalFieldKeyError::InvalidKeyLength(bytes.len()));
        }
        let metadata = fs::symlink_metadata(&path).map_err(LocalFieldKeyError::Io)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalFieldKeyError::Permissions);
        }
        let mut key = Zeroizing::new([0_u8; KEY_LENGTH]);
        key.copy_from_slice(&bytes);
        Ok(Self { id, key, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SecretEncryptionKeyProvider for LocalFieldKeyProvider {
    type Error = LocalFieldKeyError;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
        Ok(SecretEncryptionKey::new(self.id.clone(), *self.key))
    }

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        Ok((id == &self.id).then(|| SecretEncryptionKey::new(self.id.clone(), *self.key)))
    }
}

impl fmt::Display for LocalFieldKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "local field key I/O failed: {error}"),
            Self::InvalidKeyLength(length) => {
                write!(
                    formatter,
                    "local field key must be 32 bytes, found {length}"
                )
            }
            Self::Permissions => {
                write!(
                    formatter,
                    "local field key must be a regular file with mode 0600"
                )
            }
            Self::KeyId(error) => write!(formatter, "local field key id is invalid: {error}"),
            Self::Random(error) => write!(formatter, "local field key generation failed: {error}"),
        }
    }
}

impl Error for LocalFieldKeyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::KeyId(error) => Some(error),
            Self::Random(error) => Some(error),
            Self::InvalidKeyLength(_) | Self::Permissions => None,
        }
    }
}
