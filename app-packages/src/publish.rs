use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rumahl_core::InstallationId;
use sha2::{Digest, Sha256};

use crate::verifier::VerifiedPackage;

/// Root directory of the published web-asset tree, usually
/// `<state-dir>/app-assets`.
#[derive(Debug, Clone)]
pub struct WebAssetPublisherConfig {
    root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAssetPublisherConfigError {
    RootMustBeAbsolute,
}

/// Publishes verified web assets atomically per installation.
#[derive(Debug, Clone)]
pub struct WebAssetPublisher {
    config: WebAssetPublisherConfig,
}

/// Summary of one published installation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedAssets {
    installation_id: InstallationId,
    path: PathBuf,
    file_count: usize,
    total_bytes: u64,
}

#[derive(Debug)]
pub enum WebAssetPublishError {
    CreateRoot(io::Error),
    AlreadyPublished(String),
    CreateStaging(io::Error),
    SourceSymlink(String),
    SourceRead {
        path: String,
        source: io::Error,
    },
    DestinationWrite {
        path: String,
        source: io::Error,
    },
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    DigestMismatch(String),
    Commit(io::Error),
    Cleanup(io::Error),
}

impl WebAssetPublisherConfig {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, WebAssetPublisherConfigError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(WebAssetPublisherConfigError::RootMustBeAbsolute);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl WebAssetPublisher {
    pub fn new(config: WebAssetPublisherConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &WebAssetPublisherConfig {
        &self.config
    }

    /// Publishes a verified package under `<root>/<installation-id>/`.
    ///
    /// Every file is copied to a private staging directory while its digest is
    /// recomputed, so a payload swapped after verification fails closed. The
    /// staging directory is renamed into place as the final atomic step, and an
    /// existing installation directory is never overwritten.
    pub fn publish(
        &self,
        installation_id: &InstallationId,
        package_root: &Path,
        package: &VerifiedPackage,
    ) -> Result<PublishedAssets, WebAssetPublishError> {
        fs::create_dir_all(&self.config.root).map_err(WebAssetPublishError::CreateRoot)?;

        let target = self.config.root.join(installation_id.to_string());
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                return Err(WebAssetPublishError::AlreadyPublished(
                    target.to_string_lossy().into_owned(),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(WebAssetPublishError::CreateRoot(error)),
        }

        let staging = self
            .config
            .root
            .join(format!(".staging-{}", InstallationId::new()));
        fs::create_dir(&staging).map_err(WebAssetPublishError::CreateStaging)?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))
            .map_err(WebAssetPublishError::CreateStaging)?;

        match self.write_files(&staging, package_root, package) {
            Ok((file_count, total_bytes)) => {
                fs::rename(&staging, &target).map_err(WebAssetPublishError::Commit)?;
                Ok(PublishedAssets {
                    installation_id: *installation_id,
                    path: target,
                    file_count,
                    total_bytes,
                })
            }
            Err(error) => {
                if let Err(cleanup) = fs::remove_dir_all(&staging) {
                    return Err(WebAssetPublishError::Cleanup(cleanup));
                }
                Err(error)
            }
        }
    }

    fn write_files(
        &self,
        staging: &Path,
        package_root: &Path,
        package: &VerifiedPackage,
    ) -> Result<(usize, u64), WebAssetPublishError> {
        let mut total_bytes = 0_u64;

        for file in package.files() {
            let relative = file.path().as_str();
            if let Some(parent) = Path::new(relative).parent()
                && !parent.as_os_str().is_empty()
            {
                let directory = staging.join(parent);
                fs::create_dir_all(&directory).map_err(WebAssetPublishError::CreateStaging)?;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                    .map_err(WebAssetPublishError::CreateStaging)?;
            }

            total_bytes = total_bytes.saturating_add(copy_verified(
                &package_root.join(relative),
                &staging.join(relative),
                file.size(),
                file.sha256(),
                relative,
            )?);
        }

        Ok((package.files().len(), total_bytes))
    }
}

fn copy_verified(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    expected_digest: &[u8; 32],
    relative: &str,
) -> Result<u64, WebAssetPublishError> {
    let metadata =
        fs::symlink_metadata(source).map_err(|source_error| WebAssetPublishError::SourceRead {
            path: relative.to_owned(),
            source: source_error,
        })?;
    if metadata.file_type().is_symlink() {
        return Err(WebAssetPublishError::SourceSymlink(relative.to_owned()));
    }

    let mut input =
        File::open(source).map_err(|source_error| WebAssetPublishError::SourceRead {
            path: relative.to_owned(),
            source: source_error,
        })?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|source| WebAssetPublishError::DestinationWrite {
            path: relative.to_owned(),
            source,
        })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;

    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|source| WebAssetPublishError::SourceRead {
                path: relative.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }

        total = total.saturating_add(read as u64);
        if total > expected_size {
            return Err(WebAssetPublishError::SizeMismatch {
                path: relative.to_owned(),
                expected: expected_size,
                actual: total,
            });
        }

        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read]).map_err(|source| {
            WebAssetPublishError::DestinationWrite {
                path: relative.to_owned(),
                source,
            }
        })?;
    }

    if total != expected_size {
        return Err(WebAssetPublishError::SizeMismatch {
            path: relative.to_owned(),
            expected: expected_size,
            actual: total,
        });
    }

    let digest: [u8; 32] = hasher.finalize().into();
    if &digest != expected_digest {
        return Err(WebAssetPublishError::DigestMismatch(relative.to_owned()));
    }

    output
        .sync_all()
        .map_err(|source| WebAssetPublishError::DestinationWrite {
            path: relative.to_owned(),
            source,
        })?;

    Ok(total)
}

impl PublishedAssets {
    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_count(&self) -> usize {
        self.file_count
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

impl fmt::Display for WebAssetPublisherConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMustBeAbsolute => write!(f, "web asset root path must be absolute"),
        }
    }
}

impl Error for WebAssetPublisherConfigError {}

impl fmt::Display for WebAssetPublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateRoot(error) => write!(f, "web asset root could not be created: {error}"),
            Self::AlreadyPublished(path) => {
                write!(f, "web assets are already published at '{path}'")
            }
            Self::CreateStaging(error) => {
                write!(f, "web asset staging directory failed: {error}")
            }
            Self::SourceSymlink(path) => {
                write!(
                    f,
                    "package file '{path}' became a symlink before publication"
                )
            }
            Self::SourceRead { path, source } => {
                write!(f, "package file '{path}' could not be read: {source}")
            }
            Self::DestinationWrite { path, source } => {
                write!(f, "published file '{path}' could not be written: {source}")
            }
            Self::SizeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "package file '{path}' changed size during publication: {actual} bytes instead of {expected}"
            ),
            Self::DigestMismatch(path) => write!(
                f,
                "package file '{path}' changed content during publication"
            ),
            Self::Commit(error) => {
                write!(f, "published web assets could not be committed: {error}")
            }
            Self::Cleanup(error) => {
                write!(f, "failed web asset staging could not be removed: {error}")
            }
        }
    }
}

impl Error for WebAssetPublishError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateRoot(error)
            | Self::CreateStaging(error)
            | Self::Commit(error)
            | Self::Cleanup(error) => Some(error),
            Self::SourceRead { source, .. } | Self::DestinationWrite { source, .. } => Some(source),
            Self::AlreadyPublished(_)
            | Self::SourceSymlink(_)
            | Self::SizeMismatch { .. }
            | Self::DigestMismatch(_) => None,
        }
    }
}
