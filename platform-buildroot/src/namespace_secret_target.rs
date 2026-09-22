use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rumahl_oidc_provider::OidcClientSecret;
use zeroize::Zeroizing;

use crate::{
    RuntimeOidcClientSecret, RuntimeSecretRemoval, RuntimeSecretTarget, RuntimeSecretTargetOutcome,
};

const SECRET_DIRECTORY: &str = "secrets";
const CLIENT_ID_FILE: &str = "oidc-client-id";
const CLIENT_SECRET_FILE: &str = "oidc-client-secret";
const REPLAY_FILE: &str = ".oidc-client.meta";
const FORMAT_VERSION: &str = "RSM1";
const MAX_SECRET_FILE: u64 = 128;
const MAX_METADATA_FILE: u64 = 2048;

#[derive(Debug, Clone)]
pub struct NamespaceRuntimeSecretTargetConfig {
    runtime_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceRuntimeSecretTargetConfigError {
    RuntimeRootMustBeAbsolute,
}

#[derive(Debug, Clone)]
pub struct NamespaceRuntimeSecretTarget {
    config: NamespaceRuntimeSecretTargetConfig,
}

#[derive(Debug)]
pub enum NamespaceRuntimeSecretTargetError {
    Inspect(io::Error),
    InvalidDirectory,
    OwnerMismatch { expected: u32, actual: u32 },
    WritableByNonOwner(u32),
    Open(io::Error),
    InvalidFile,
    FileOwnerMismatch { expected: u32, actual: u32 },
    FileWritableByNonOwner(u32),
    Read(io::Error),
    FileTooLarge,
    Write(io::Error),
    Rename(io::Error),
    Remove(io::Error),
}

impl NamespaceRuntimeSecretTargetConfig {
    pub fn new(
        runtime_root: impl Into<PathBuf>,
    ) -> Result<Self, NamespaceRuntimeSecretTargetConfigError> {
        let runtime_root = runtime_root.into();
        if !runtime_root.is_absolute() {
            return Err(NamespaceRuntimeSecretTargetConfigError::RuntimeRootMustBeAbsolute);
        }
        Ok(Self { runtime_root })
    }

    pub fn runtime_root(&self) -> &Path {
        &self.runtime_root
    }
}

impl NamespaceRuntimeSecretTarget {
    pub fn new(config: NamespaceRuntimeSecretTargetConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &NamespaceRuntimeSecretTargetConfig {
        &self.config
    }

    fn installation_root(&self, request: &RuntimeOidcClientSecret) -> PathBuf {
        self.config
            .runtime_root
            .join(request.installation_id().to_string())
    }

    fn removal_root(&self, request: &RuntimeSecretRemoval) -> PathBuf {
        self.config
            .runtime_root
            .join(request.installation_id().to_string())
    }
}

impl RuntimeSecretTarget for NamespaceRuntimeSecretTarget {
    type Error = NamespaceRuntimeSecretTargetError;

    fn deliver_oidc_client_secret(
        &self,
        request: RuntimeOidcClientSecret,
    ) -> Result<RuntimeSecretTargetOutcome, Self::Error> {
        validate_directory(&self.config.runtime_root)?;
        let installation_root = self.installation_root(&request);
        validate_directory(&installation_root)?;
        let secret_root = installation_root.join(SECRET_DIRECTORY);
        validate_directory(&secret_root)?;

        let encoded_secret = request.client_secret().encode();
        let expected_metadata = Zeroizing::new(format!(
            "{FORMAT_VERSION}\n{}\n{}\n{}\n{}\n{}\n",
            request.operation_id(),
            request.app_id(),
            request.publisher_id(),
            request.client_id(),
            hex(request.client_secret().digest().as_bytes())
        ));
        let secret_path = secret_root.join(CLIENT_SECRET_FILE);
        let client_path = secret_root.join(CLIENT_ID_FILE);
        let replay_path = installation_root.join(REPLAY_FILE);

        let existing_secret = read_optional(&secret_path, MAX_SECRET_FILE)?;
        let existing_client = read_optional(&client_path, MAX_SECRET_FILE)?;
        let existing_metadata = read_optional(&replay_path, MAX_METADATA_FILE)?;

        if let Some(secret) = existing_secret.as_deref() {
            let secret = std::str::from_utf8(secret)
                .ok()
                .and_then(|value| OidcClientSecret::parse(value).ok());
            if secret
                .as_ref()
                .is_none_or(|secret| !request.client_secret().digest().verifies(secret))
            {
                return Ok(RuntimeSecretTargetOutcome::Conflict);
            }
        }
        if existing_client
            .as_deref()
            .is_some_and(|value| value != request.client_id().as_str().as_bytes())
            || existing_metadata
                .as_deref()
                .is_some_and(|value| value != expected_metadata.as_bytes())
        {
            return Ok(RuntimeSecretTargetOutcome::Conflict);
        }

        let complete =
            existing_secret.is_some() && existing_client.is_some() && existing_metadata.is_some();
        if existing_secret.is_none() {
            write_atomic(
                &secret_root,
                CLIENT_SECRET_FILE,
                encoded_secret.as_bytes(),
                0o444,
            )?;
        }
        if existing_client.is_none() {
            write_atomic(
                &secret_root,
                CLIENT_ID_FILE,
                request.client_id().as_str().as_bytes(),
                0o444,
            )?;
        }
        if existing_metadata.is_none() {
            write_atomic(
                &installation_root,
                REPLAY_FILE,
                expected_metadata.as_bytes(),
                0o600,
            )?;
        }

        Ok(if complete {
            RuntimeSecretTargetOutcome::AlreadyApplied
        } else {
            RuntimeSecretTargetOutcome::Applied
        })
    }

    fn remove_for_installation(
        &self,
        request: RuntimeSecretRemoval,
    ) -> Result<RuntimeSecretTargetOutcome, Self::Error> {
        validate_directory(&self.config.runtime_root)?;
        let installation_root = self.removal_root(&request);
        match fs::symlink_metadata(&installation_root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RuntimeSecretTargetOutcome::AlreadyApplied);
            }
            Err(error) => return Err(NamespaceRuntimeSecretTargetError::Inspect(error)),
            Ok(_) => validate_directory(&installation_root)?,
        }
        let secret_root = installation_root.join(SECRET_DIRECTORY);
        validate_directory(&secret_root)?;

        let mut removed = false;
        for path in [
            secret_root.join(CLIENT_SECRET_FILE),
            secret_root.join(CLIENT_ID_FILE),
            installation_root.join(REPLAY_FILE),
            secret_root.join(temp_name(CLIENT_SECRET_FILE)),
            secret_root.join(temp_name(CLIENT_ID_FILE)),
            installation_root.join(temp_name(REPLAY_FILE)),
        ] {
            removed |= remove_optional(&path)?;
        }

        Ok(if removed {
            RuntimeSecretTargetOutcome::Applied
        } else {
            RuntimeSecretTargetOutcome::AlreadyApplied
        })
    }
}

fn validate_directory(path: &Path) -> Result<(), NamespaceRuntimeSecretTargetError> {
    let metadata =
        fs::symlink_metadata(path).map_err(NamespaceRuntimeSecretTargetError::Inspect)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(NamespaceRuntimeSecretTargetError::InvalidDirectory);
    }
    let expected = current_uid();
    if metadata.uid() != expected {
        return Err(NamespaceRuntimeSecretTargetError::OwnerMismatch {
            expected,
            actual: metadata.uid(),
        });
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(NamespaceRuntimeSecretTargetError::WritableByNonOwner(mode));
    }
    Ok(())
}

fn read_optional(
    path: &Path,
    maximum: u64,
) -> Result<Option<Zeroizing<Vec<u8>>>, NamespaceRuntimeSecretTargetError> {
    let mut file = match open_checked(path) {
        Ok(file) => file,
        Err(NamespaceRuntimeSecretTargetError::Open(error))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let mut bytes = Zeroizing::new(Vec::with_capacity(maximum as usize));
    Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(NamespaceRuntimeSecretTargetError::Read)?;
    if bytes.len() > maximum as usize {
        return Err(NamespaceRuntimeSecretTargetError::FileTooLarge);
    }
    Ok(Some(bytes))
}

fn open_checked(path: &Path) -> Result<File, NamespaceRuntimeSecretTargetError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(NamespaceRuntimeSecretTargetError::Open)?;
    let metadata = file
        .metadata()
        .map_err(NamespaceRuntimeSecretTargetError::Inspect)?;
    if !metadata.file_type().is_file() {
        return Err(NamespaceRuntimeSecretTargetError::InvalidFile);
    }
    let expected = current_uid();
    if metadata.uid() != expected {
        return Err(NamespaceRuntimeSecretTargetError::FileOwnerMismatch {
            expected,
            actual: metadata.uid(),
        });
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(NamespaceRuntimeSecretTargetError::FileWritableByNonOwner(
            mode,
        ));
    }
    Ok(file)
}

fn write_atomic(
    directory: &Path,
    name: &str,
    value: &[u8],
    mode: u32,
) -> Result<(), NamespaceRuntimeSecretTargetError> {
    let temporary = directory.join(temp_name(name));
    remove_optional(&temporary)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(NamespaceRuntimeSecretTargetError::Write)?;
    file.write_all(value)
        .and_then(|()| file.sync_all())
        .map_err(NamespaceRuntimeSecretTargetError::Write)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(mode))
        .map_err(NamespaceRuntimeSecretTargetError::Write)?;
    fs::rename(&temporary, directory.join(name)).map_err(NamespaceRuntimeSecretTargetError::Rename)
}

fn remove_optional(path: &Path) -> Result<bool, NamespaceRuntimeSecretTargetError> {
    match open_checked(path) {
        Ok(file) => drop(file),
        Err(NamespaceRuntimeSecretTargetError::Open(error))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    }
    fs::remove_file(path)
        .map(|()| true)
        .map_err(NamespaceRuntimeSecretTargetError::Remove)
}

fn temp_name(name: &str) -> String {
    format!(".{name}.tmp")
}

fn current_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions.
    unsafe { libc::geteuid() }
}

fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

impl fmt::Display for NamespaceRuntimeSecretTargetConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime secret root path must be absolute")
    }
}

impl Error for NamespaceRuntimeSecretTargetConfigError {}

impl fmt::Display for NamespaceRuntimeSecretTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspect(_) => write!(f, "runtime secret path inspection failed"),
            Self::InvalidDirectory => write!(f, "runtime secret path is not a trusted directory"),
            Self::OwnerMismatch { .. } => write!(f, "runtime secret directory owner mismatch"),
            Self::WritableByNonOwner(_) => write!(
                f,
                "runtime secret directory is writable by another principal"
            ),
            Self::Open(_) => write!(f, "runtime secret file open failed"),
            Self::InvalidFile => write!(f, "runtime secret path is not a regular file"),
            Self::FileOwnerMismatch { .. } => write!(f, "runtime secret file owner mismatch"),
            Self::FileWritableByNonOwner(_) => {
                write!(f, "runtime secret file is writable by another principal")
            }
            Self::Read(_) => write!(f, "runtime secret file read failed"),
            Self::FileTooLarge => write!(f, "runtime secret file is too large"),
            Self::Write(_) => write!(f, "runtime secret file write failed"),
            Self::Rename(_) => write!(f, "runtime secret publication failed"),
            Self::Remove(_) => write!(f, "runtime secret removal failed"),
        }
    }
}

impl Error for NamespaceRuntimeSecretTargetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspect(error)
            | Self::Open(error)
            | Self::Read(error)
            | Self::Write(error)
            | Self::Rename(error)
            | Self::Remove(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rumahl_core::{AppId, AppOperationId, InstallationId, PublisherId};
    use rumahl_oidc_provider::{OidcClientId, OidcClientSecret};

    use super::*;
    use crate::test_support::unique_test_root;

    fn target_and_namespace() -> (PathBuf, NamespaceRuntimeSecretTarget, InstallationId) {
        let root = unique_test_root('n');
        let installation_id = InstallationId::new();
        let installation = root.join(installation_id.to_string());
        fs::create_dir(&installation).unwrap();
        fs::create_dir(installation.join(SECRET_DIRECTORY)).unwrap();
        let target = NamespaceRuntimeSecretTarget::new(
            NamespaceRuntimeSecretTargetConfig::new(&root).unwrap(),
        );
        (root, target, installation_id)
    }

    fn delivery(
        installation_id: InstallationId,
        secret: OidcClientSecret,
    ) -> RuntimeOidcClientSecret {
        RuntimeOidcClientSecret::new(
            AppOperationId::new(),
            installation_id,
            AppId::parse("com.rumahl.test").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            OidcClientId::generate().unwrap(),
            secret,
        )
    }

    #[test]
    fn delivers_replays_and_removes_oidc_material() {
        let (root, target, installation_id) = target_and_namespace();
        let request = delivery(installation_id, OidcClientSecret::generate().unwrap());
        let operation_id = *request.operation_id();
        let encoded = request.client_secret().encode();
        let client_id = request.client_id().as_str().to_owned();

        assert_eq!(
            target.deliver_oidc_client_secret(request).unwrap(),
            RuntimeSecretTargetOutcome::Applied
        );
        let secret_root = root
            .join(installation_id.to_string())
            .join(SECRET_DIRECTORY);
        assert_eq!(
            fs::read_to_string(secret_root.join(CLIENT_SECRET_FILE)).unwrap(),
            encoded.as_str()
        );
        assert_eq!(
            fs::read_to_string(secret_root.join(CLIENT_ID_FILE)).unwrap(),
            client_id
        );

        let replay = RuntimeOidcClientSecret::new(
            operation_id,
            installation_id,
            AppId::parse("com.rumahl.test").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            OidcClientId::parse(client_id).unwrap(),
            OidcClientSecret::parse(&encoded).unwrap(),
        );
        assert_eq!(
            target.deliver_oidc_client_secret(replay).unwrap(),
            RuntimeSecretTargetOutcome::AlreadyApplied
        );
        assert_eq!(
            target
                .remove_for_installation(RuntimeSecretRemoval::new(
                    AppOperationId::new(),
                    installation_id
                ))
                .unwrap(),
            RuntimeSecretTargetOutcome::Applied
        );
        assert!(fs::read_dir(&secret_root).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_changed_replay_and_missing_namespace() {
        let (root, target, installation_id) = target_and_namespace();
        let first = delivery(installation_id, OidcClientSecret::generate().unwrap());
        let operation_id = *first.operation_id();
        let client_id = first.client_id().as_str().to_owned();
        target.deliver_oidc_client_secret(first).unwrap();
        let changed = RuntimeOidcClientSecret::new(
            operation_id,
            installation_id,
            AppId::parse("com.rumahl.test").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            OidcClientId::parse(client_id).unwrap(),
            OidcClientSecret::generate().unwrap(),
        );
        assert_eq!(
            target.deliver_oidc_client_secret(changed).unwrap(),
            RuntimeSecretTargetOutcome::Conflict
        );
        let absent = delivery(InstallationId::new(), OidcClientSecret::generate().unwrap());
        assert!(target.deliver_oidc_client_secret(absent).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
