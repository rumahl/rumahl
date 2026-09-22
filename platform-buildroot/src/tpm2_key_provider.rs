use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rumahl_persistence_sqlite::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
};
use zeroize::Zeroizing;

const ROOT_KEY_LENGTH: usize = 32;
const MAX_UNSEAL_OUTPUT_LENGTH: u64 = ROOT_KEY_LENGTH as u64 + 1;
const DEFAULT_UNSEAL_TIMEOUT: Duration = Duration::from_secs(10);
const WAIT_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tpm2Authorization {
    Passwordless,
    PolicySession(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tpm2SealedKey {
    id: SecretEncryptionKeyId,
    object_context: PathBuf,
    authorization: Tpm2Authorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tpm2SealedKeyError {
    ObjectContextMustBeAbsolute,
    PolicySessionMustBeAbsolute,
}

#[derive(Debug)]
pub struct Tpm2UnsealKeyProvider {
    executable: PathBuf,
    active_key_id: SecretEncryptionKeyId,
    keys: HashMap<SecretEncryptionKeyId, Tpm2SealedKey>,
    timeout: Duration,
}

#[derive(Debug)]
pub enum Tpm2UnsealKeyProviderError {
    ExecutableMustBeAbsolute,
    ZeroTimeout,
    EmptyKeySet,
    DuplicateKeyId(SecretEncryptionKeyId),
    ActiveKeyMissing(SecretEncryptionKeyId),
    Spawn(io::Error),
    Read(io::Error),
    Wait(io::Error),
    Terminate(io::Error),
    ReaderPanicked,
    TimedOut,
    UnsealFailed(Option<i32>),
    InvalidKeyLength(usize),
}

impl Tpm2SealedKey {
    pub fn new(
        id: SecretEncryptionKeyId,
        object_context: impl Into<PathBuf>,
        authorization: Tpm2Authorization,
    ) -> Result<Self, Tpm2SealedKeyError> {
        let object_context = object_context.into();
        if !object_context.is_absolute() {
            return Err(Tpm2SealedKeyError::ObjectContextMustBeAbsolute);
        }
        if let Tpm2Authorization::PolicySession(path) = &authorization
            && !path.is_absolute()
        {
            return Err(Tpm2SealedKeyError::PolicySessionMustBeAbsolute);
        }

        Ok(Self {
            id,
            object_context,
            authorization,
        })
    }

    pub fn id(&self) -> &SecretEncryptionKeyId {
        &self.id
    }

    pub fn object_context(&self) -> &Path {
        &self.object_context
    }

    pub fn authorization(&self) -> &Tpm2Authorization {
        &self.authorization
    }
}

impl Tpm2UnsealKeyProvider {
    pub fn new(
        executable: impl Into<PathBuf>,
        active_key_id: SecretEncryptionKeyId,
        sealed_keys: impl IntoIterator<Item = Tpm2SealedKey>,
    ) -> Result<Self, Tpm2UnsealKeyProviderError> {
        Self::with_timeout(
            executable,
            active_key_id,
            sealed_keys,
            DEFAULT_UNSEAL_TIMEOUT,
        )
    }

    pub fn with_timeout(
        executable: impl Into<PathBuf>,
        active_key_id: SecretEncryptionKeyId,
        sealed_keys: impl IntoIterator<Item = Tpm2SealedKey>,
        timeout: Duration,
    ) -> Result<Self, Tpm2UnsealKeyProviderError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(Tpm2UnsealKeyProviderError::ExecutableMustBeAbsolute);
        }
        if timeout.is_zero() {
            return Err(Tpm2UnsealKeyProviderError::ZeroTimeout);
        }

        let mut keys = HashMap::new();
        for key in sealed_keys {
            let id = key.id.clone();
            if keys.insert(id.clone(), key).is_some() {
                return Err(Tpm2UnsealKeyProviderError::DuplicateKeyId(id));
            }
        }
        if keys.is_empty() {
            return Err(Tpm2UnsealKeyProviderError::EmptyKeySet);
        }
        if !keys.contains_key(&active_key_id) {
            return Err(Tpm2UnsealKeyProviderError::ActiveKeyMissing(active_key_id));
        }

        Ok(Self {
            executable,
            active_key_id,
            keys,
            timeout,
        })
    }

    fn unseal(
        &self,
        sealed_key: &Tpm2SealedKey,
    ) -> Result<SecretEncryptionKey, Tpm2UnsealKeyProviderError> {
        let mut command = Command::new(&self.executable);
        command
            .arg("--object-context")
            .arg(&sealed_key.object_context)
            .arg("--quiet")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Tpm2Authorization::PolicySession(path) = &sealed_key.authorization {
            let mut authorization = std::ffi::OsString::from("session:");
            authorization.push(path);
            command.arg("--auth").arg(authorization);
        }

        let mut child = command.spawn().map_err(Tpm2UnsealKeyProviderError::Spawn)?;
        let stdout = child.stdout.take().expect("stdout is configured as piped");
        let reader = thread::spawn(move || {
            let mut value = Zeroizing::new(Vec::with_capacity(MAX_UNSEAL_OUTPUT_LENGTH as usize));
            stdout
                .take(MAX_UNSEAL_OUTPUT_LENGTH)
                .read_to_end(&mut value)?;
            Ok::<_, io::Error>(value)
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(WAIT_INTERVAL),
                Ok(None) => {
                    let terminate_result = child.kill();
                    let wait_result = child.wait();
                    let reader_result = join_reader(reader);
                    terminate_result.map_err(Tpm2UnsealKeyProviderError::Terminate)?;
                    wait_result.map_err(Tpm2UnsealKeyProviderError::Wait)?;
                    drop(reader_result?);
                    return Err(Tpm2UnsealKeyProviderError::TimedOut);
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drop(join_reader(reader)?);
                    return Err(Tpm2UnsealKeyProviderError::Wait(error));
                }
            }
        };
        let value = join_reader(reader)?;
        if !status.success() {
            return Err(Tpm2UnsealKeyProviderError::UnsealFailed(status.code()));
        }
        if value.len() != ROOT_KEY_LENGTH {
            return Err(Tpm2UnsealKeyProviderError::InvalidKeyLength(value.len()));
        }

        let mut key = [0_u8; ROOT_KEY_LENGTH];
        key.copy_from_slice(&value);
        Ok(SecretEncryptionKey::new(sealed_key.id.clone(), key))
    }
}

fn join_reader(
    reader: thread::JoinHandle<io::Result<Zeroizing<Vec<u8>>>>,
) -> Result<Zeroizing<Vec<u8>>, Tpm2UnsealKeyProviderError> {
    reader
        .join()
        .map_err(|_| Tpm2UnsealKeyProviderError::ReaderPanicked)?
        .map_err(Tpm2UnsealKeyProviderError::Read)
}

impl SecretEncryptionKeyProvider for Tpm2UnsealKeyProvider {
    type Error = Tpm2UnsealKeyProviderError;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
        let key = self
            .keys
            .get(&self.active_key_id)
            .expect("constructor validates active key");
        self.unseal(key)
    }

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        self.keys.get(id).map(|key| self.unseal(key)).transpose()
    }
}

impl fmt::Display for Tpm2SealedKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectContextMustBeAbsolute => {
                write!(f, "TPM object context path must be absolute")
            }
            Self::PolicySessionMustBeAbsolute => {
                write!(f, "TPM policy session path must be absolute")
            }
        }
    }
}

impl Error for Tpm2SealedKeyError {}

impl fmt::Display for Tpm2UnsealKeyProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutableMustBeAbsolute => write!(f, "tpm2_unseal path must be absolute"),
            Self::ZeroTimeout => write!(f, "tpm2_unseal timeout must be non-zero"),
            Self::EmptyKeySet => write!(f, "at least one TPM-sealed key is required"),
            Self::DuplicateKeyId(id) => write!(f, "duplicate TPM-sealed key ID {id}"),
            Self::ActiveKeyMissing(id) => write!(f, "active TPM-sealed key {id} is not configured"),
            Self::Spawn(_) => write!(f, "failed to execute tpm2_unseal"),
            Self::Read(_) => write!(f, "failed to read tpm2_unseal output"),
            Self::Wait(_) => write!(f, "failed while waiting for tpm2_unseal"),
            Self::Terminate(_) => write!(f, "failed to terminate timed-out tpm2_unseal"),
            Self::ReaderPanicked => write!(f, "tpm2_unseal output reader failed"),
            Self::TimedOut => write!(f, "tpm2_unseal timed out"),
            Self::UnsealFailed(code) => write!(f, "tpm2_unseal failed with status {code:?}"),
            Self::InvalidKeyLength(length) => {
                write!(f, "TPM-unsealed root key has invalid length {length}")
            }
        }
    }
}

impl Error for Tpm2UnsealKeyProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Read(error) | Self::Wait(error) | Self::Terminate(error) => {
                Some(error)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_core::{
        AppId, AppIdentity, InstallationId, PublisherId, SecretPurpose, SecretRecord, SecretStore,
        SecretValue, UnixTimestamp,
    };
    use rumahl_persistence_sqlite::SqliteSecretStore;

    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rumahl-{name}-{}-{nonce}", std::process::id()))
    }

    fn fake_unseal(root: &Path, output: &str) -> PathBuf {
        let executable = root.join("tpm2_unseal");
        fs::write(&executable, format!("#!/bin/sh\nprintf '%s' '{output}'\n")).unwrap();
        make_executable(&executable);
        executable
    }

    fn make_executable(executable: &Path) {
        let mut permissions = fs::metadata(executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(executable, permissions).unwrap();
    }

    #[test]
    fn unseals_active_and_historical_keys_by_id() {
        let root = test_root("tpm2-provider");
        fs::create_dir(&root).unwrap();
        let executable = fake_unseal(&root, "01234567890123456789012345678901");
        let active_id = SecretEncryptionKeyId::parse("device-key-2").unwrap();
        let old_id = SecretEncryptionKeyId::parse("device-key-1").unwrap();
        let provider = Tpm2UnsealKeyProvider::new(
            executable,
            active_id.clone(),
            [
                Tpm2SealedKey::new(
                    old_id.clone(),
                    root.join("old.ctx"),
                    Tpm2Authorization::Passwordless,
                )
                .unwrap(),
                Tpm2SealedKey::new(
                    active_id.clone(),
                    root.join("active.ctx"),
                    Tpm2Authorization::PolicySession(root.join("policy.session")),
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(provider.active_key().unwrap().id(), &active_id);
        assert_eq!(provider.key_by_id(&old_id).unwrap().unwrap().id(), &old_id);
        assert!(
            provider
                .key_by_id(&SecretEncryptionKeyId::parse("unknown").unwrap())
                .unwrap()
                .is_none()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsealed_values_that_are_not_aes_256_keys() {
        let root = test_root("tpm2-provider-length");
        fs::create_dir(&root).unwrap();
        let executable = fake_unseal(&root, "too-short");
        let id = SecretEncryptionKeyId::parse("device-key-1").unwrap();
        let provider = Tpm2UnsealKeyProvider::new(
            executable,
            id.clone(),
            [
                Tpm2SealedKey::new(id, root.join("key.ctx"), Tpm2Authorization::Passwordless)
                    .unwrap(),
            ],
        )
        .unwrap();

        assert!(matches!(
            provider.active_key(),
            Err(Tpm2UnsealKeyProviderError::InvalidKeyLength(9))
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn decrypts_sqlite_secrets_with_the_same_tpm_sealed_key() {
        let root = test_root("tpm2-provider-store");
        fs::create_dir(&root).unwrap();
        let executable = fake_unseal(&root, "01234567890123456789012345678901");
        let id = SecretEncryptionKeyId::parse("device-key-1").unwrap();
        let provider = Tpm2UnsealKeyProvider::new(
            executable,
            id.clone(),
            [
                Tpm2SealedKey::new(id, root.join("key.ctx"), Tpm2Authorization::Passwordless)
                    .unwrap(),
            ],
        )
        .unwrap();
        let store = SqliteSecretStore::open_in_memory(provider).unwrap();
        let owner = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );
        let purpose = SecretPurpose::parse("rumahl.test.secret").unwrap();
        let record = SecretRecord::new(
            owner.clone(),
            purpose.clone(),
            SecretValue::new(b"runtime-value".to_vec()).unwrap(),
            UnixTimestamp::from_seconds(42),
        );

        store.insert(&record).unwrap();
        let recovered = store
            .find_by_owner_and_purpose(&owner, &purpose)
            .unwrap()
            .unwrap();

        assert_eq!(recovered.value().as_bytes(), b"runtime-value");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn requires_absolute_security_object_paths() {
        let id = SecretEncryptionKeyId::parse("device-key-1").unwrap();

        assert_eq!(
            Tpm2SealedKey::new(id.clone(), "relative.ctx", Tpm2Authorization::Passwordless,)
                .unwrap_err(),
            Tpm2SealedKeyError::ObjectContextMustBeAbsolute
        );
        assert_eq!(
            Tpm2SealedKey::new(
                id,
                "/var/lib/rumahl/key.ctx",
                Tpm2Authorization::PolicySession(PathBuf::from("session.ctx")),
            )
            .unwrap_err(),
            Tpm2SealedKeyError::PolicySessionMustBeAbsolute
        );
    }

    #[test]
    fn terminates_a_hung_unseal_process() {
        let root = test_root("tpm2-provider-timeout");
        fs::create_dir(&root).unwrap();
        let executable = root.join("tpm2_unseal");
        fs::write(&executable, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
        make_executable(&executable);
        let id = SecretEncryptionKeyId::parse("device-key-1").unwrap();
        let provider = Tpm2UnsealKeyProvider::with_timeout(
            executable,
            id.clone(),
            [
                Tpm2SealedKey::new(id, root.join("key.ctx"), Tpm2Authorization::Passwordless)
                    .unwrap(),
            ],
            Duration::from_millis(25),
        )
        .unwrap();

        assert!(matches!(
            provider.active_key(),
            Err(Tpm2UnsealKeyProviderError::TimedOut)
        ));

        fs::remove_dir_all(root).unwrap();
    }
}
