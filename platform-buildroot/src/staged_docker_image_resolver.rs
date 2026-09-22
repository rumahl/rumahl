use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rumahl_core::PackagePath;

use crate::{DockerImageReference, DockerImageResolver, RuntimeInstallationSpec};

const METADATA_FILE: &str = "image-reference";
const FORMAT_VERSION: &str = "RDI1";
const MAX_METADATA_LENGTH: u64 = 2048;

#[derive(Debug, Clone)]
pub struct StagedDockerImageResolverConfig {
    root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedDockerImageResolverConfigError {
    RootMustBeAbsolute,
}

#[derive(Debug, Clone)]
pub struct StagedDockerImageResolver {
    config: StagedDockerImageResolverConfig,
}

#[derive(Debug)]
pub enum StagedDockerImageResolverError {
    Root(io::Error),
    RootIsNotDirectory,
    RootOwnerMismatch { expected: u32, actual: u32 },
    RootPermissions(u32),
    InstallationDirectory(io::Error),
    InstallationPathIsNotDirectory,
    InstallationOwnerMismatch { expected: u32, actual: u32 },
    InstallationPermissions(u32),
    Open(io::Error),
    Metadata(io::Error),
    MetadataIsNotRegularFile,
    MetadataOwnerMismatch { expected: u32, actual: u32 },
    MetadataPermissions(u32),
    Read(io::Error),
    MetadataTooLarge,
    InvalidMetadata,
    ArtifactMismatch,
    InvalidImageReference,
}

impl StagedDockerImageResolverConfig {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StagedDockerImageResolverConfigError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(StagedDockerImageResolverConfigError::RootMustBeAbsolute);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl StagedDockerImageResolver {
    pub fn new(config: StagedDockerImageResolverConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &StagedDockerImageResolverConfig {
        &self.config
    }

    fn validate_directory(
        &self,
        path: &Path,
        root: bool,
    ) -> Result<(), StagedDockerImageResolverError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if root {
                StagedDockerImageResolverError::Root(error)
            } else {
                StagedDockerImageResolverError::InstallationDirectory(error)
            }
        })?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(if root {
                StagedDockerImageResolverError::RootIsNotDirectory
            } else {
                StagedDockerImageResolverError::InstallationPathIsNotDirectory
            });
        }
        let expected = current_uid();
        if metadata.uid() != expected {
            return Err(if root {
                StagedDockerImageResolverError::RootOwnerMismatch {
                    expected,
                    actual: metadata.uid(),
                }
            } else {
                StagedDockerImageResolverError::InstallationOwnerMismatch {
                    expected,
                    actual: metadata.uid(),
                }
            });
        }
        let mode = metadata.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(if root {
                StagedDockerImageResolverError::RootPermissions(mode)
            } else {
                StagedDockerImageResolverError::InstallationPermissions(mode)
            });
        }
        Ok(())
    }

    fn read_metadata(
        &self,
        spec: &RuntimeInstallationSpec,
    ) -> Result<String, StagedDockerImageResolverError> {
        self.validate_directory(&self.config.root, true)?;
        let installation_root = self
            .config
            .root
            .join(spec.identity().installation_id().to_string());
        self.validate_directory(&installation_root, false)?;
        let path = installation_root.join(METADATA_FILE);
        let file = open_regular_file(&path)?;
        let metadata = file
            .metadata()
            .map_err(StagedDockerImageResolverError::Metadata)?;
        let expected = current_uid();
        if metadata.uid() != expected {
            return Err(StagedDockerImageResolverError::MetadataOwnerMismatch {
                expected,
                actual: metadata.uid(),
            });
        }
        let mode = metadata.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(StagedDockerImageResolverError::MetadataPermissions(mode));
        }
        let mut bytes = Vec::with_capacity(MAX_METADATA_LENGTH as usize);
        file.take(MAX_METADATA_LENGTH + 1)
            .read_to_end(&mut bytes)
            .map_err(StagedDockerImageResolverError::Read)?;
        if bytes.len() > MAX_METADATA_LENGTH as usize {
            return Err(StagedDockerImageResolverError::MetadataTooLarge);
        }
        String::from_utf8(bytes).map_err(|_| StagedDockerImageResolverError::InvalidMetadata)
    }
}

impl DockerImageResolver for StagedDockerImageResolver {
    type Error = StagedDockerImageResolverError;

    fn resolve_image(
        &self,
        spec: &RuntimeInstallationSpec,
        artifact: &PackagePath,
    ) -> Result<DockerImageReference, Self::Error> {
        let metadata = self.read_metadata(spec)?;
        let fields = metadata.split('\n').collect::<Vec<_>>();
        if fields.len() != 4 || fields[0] != FORMAT_VERSION || !fields[3].is_empty() {
            return Err(StagedDockerImageResolverError::InvalidMetadata);
        }
        if fields[1] != artifact.as_str() {
            return Err(StagedDockerImageResolverError::ArtifactMismatch);
        }
        DockerImageReference::parse(fields[2].to_owned())
            .map_err(|_| StagedDockerImageResolverError::InvalidImageReference)
    }
}

fn open_regular_file(path: &Path) -> Result<File, StagedDockerImageResolverError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(StagedDockerImageResolverError::Open)?;
    let metadata = file
        .metadata()
        .map_err(StagedDockerImageResolverError::Metadata)?;
    if !metadata.file_type().is_file() {
        return Err(StagedDockerImageResolverError::MetadataIsNotRegularFile);
    }
    Ok(file)
}

fn current_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions.
    unsafe { libc::geteuid() }
}

impl fmt::Display for StagedDockerImageResolverConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMustBeAbsolute => write!(f, "staged image root path must be absolute"),
        }
    }
}

impl Error for StagedDockerImageResolverConfigError {}

impl fmt::Display for StagedDockerImageResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(_) => write!(f, "staged image root inspection failed"),
            Self::RootIsNotDirectory => {
                write!(f, "staged image root is not a non-symlink directory")
            }
            Self::RootOwnerMismatch { expected, actual } => write!(
                f,
                "staged image root owner UID {actual} does not match supervisor UID {expected}"
            ),
            Self::RootPermissions(mode) => write!(
                f,
                "staged image root permissions {mode:o} allow non-owner writes"
            ),
            Self::InstallationDirectory(_) => {
                write!(f, "staged installation directory inspection failed")
            }
            Self::InstallationPathIsNotDirectory => {
                write!(f, "staged installation path is not a non-symlink directory")
            }
            Self::InstallationOwnerMismatch { expected, actual } => write!(
                f,
                "staged installation owner UID {actual} does not match supervisor UID {expected}"
            ),
            Self::InstallationPermissions(mode) => write!(
                f,
                "staged installation permissions {mode:o} allow non-owner writes"
            ),
            Self::Open(_) => write!(f, "staged image metadata open failed"),
            Self::Metadata(_) => write!(f, "staged image metadata inspection failed"),
            Self::MetadataIsNotRegularFile => {
                write!(f, "staged image metadata is not a regular file")
            }
            Self::MetadataOwnerMismatch { expected, actual } => write!(
                f,
                "staged image metadata owner UID {actual} does not match supervisor UID {expected}"
            ),
            Self::MetadataPermissions(mode) => write!(
                f,
                "staged image metadata permissions {mode:o} allow non-owner writes"
            ),
            Self::Read(_) => write!(f, "staged image metadata read failed"),
            Self::MetadataTooLarge => write!(f, "staged image metadata is too large"),
            Self::InvalidMetadata => write!(f, "staged image metadata is invalid"),
            Self::ArtifactMismatch => {
                write!(
                    f,
                    "staged image does not match the declared package artifact"
                )
            }
            Self::InvalidImageReference => {
                write!(
                    f,
                    "staged image reference is not an immutable SHA-256 reference"
                )
            }
        }
    }
}

impl Error for StagedDockerImageResolverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root(error)
            | Self::InstallationDirectory(error)
            | Self::Open(error)
            | Self::Metadata(error)
            | Self::Read(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use rumahl_core::{
        AppId, AppIdentity, AppVersion, InstallationId, PublisherId, RuntimeDescriptor,
    };

    use super::*;
    use crate::test_support::unique_test_root;

    fn spec() -> RuntimeInstallationSpec {
        RuntimeInstallationSpec::new(
            AppIdentity::new(
                AppId::parse("com.rumahl.test").unwrap(),
                InstallationId::new(),
                PublisherId::parse("com.rumahl").unwrap(),
            ),
            AppVersion::new(1, 0, 0),
            RuntimeDescriptor::container(),
        )
    }

    fn staged_resolver(
        metadata: &str,
    ) -> (PathBuf, StagedDockerImageResolver, RuntimeInstallationSpec) {
        let root = unique_test_root('i');
        let spec = spec();
        let installation_root = root.join(spec.identity().installation_id().to_string());
        fs::create_dir(&installation_root).unwrap();
        fs::write(installation_root.join(METADATA_FILE), metadata).unwrap();
        let resolver =
            StagedDockerImageResolver::new(StagedDockerImageResolverConfig::new(&root).unwrap());
        (root, resolver, spec)
    }

    #[test]
    fn resolves_matching_immutable_staged_image() {
        let digest = "a".repeat(64);
        let metadata = format!("RDI1\nruntime/server.oci\nsha256:{digest}\n");
        let (root, resolver, spec) = staged_resolver(&metadata);

        let image = resolver
            .resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap())
            .unwrap();

        assert_eq!(image.as_str(), format!("sha256:{digest}"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_artifact_mismatch_and_mutable_reference() {
        let digest = "b".repeat(64);
        let metadata = format!("RDI1\nruntime/other.oci\nsha256:{digest}\n");
        let (root, resolver, spec) = staged_resolver(&metadata);
        assert!(matches!(
            resolver.resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap()),
            Err(StagedDockerImageResolverError::ArtifactMismatch)
        ));
        fs::remove_dir_all(&root).unwrap();

        let (root, resolver, spec) =
            staged_resolver("RDI1\nruntime/server.oci\nregistry/app:latest\n");
        assert!(matches!(
            resolver.resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap()),
            Err(StagedDockerImageResolverError::InvalidImageReference)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_symlinked_or_writable_metadata() {
        let root = unique_test_root('i');
        let spec = spec();
        let installation_root = root.join(spec.identity().installation_id().to_string());
        fs::create_dir(&installation_root).unwrap();
        let outside = root.join("outside");
        fs::write(&outside, "RDI1\nruntime/server.oci\nsha256:00\n").unwrap();
        symlink(&outside, installation_root.join(METADATA_FILE)).unwrap();
        let resolver =
            StagedDockerImageResolver::new(StagedDockerImageResolverConfig::new(&root).unwrap());
        assert!(matches!(
            resolver.resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap()),
            Err(StagedDockerImageResolverError::Open(_))
        ));
        fs::remove_file(installation_root.join(METADATA_FILE)).unwrap();
        fs::write(
            installation_root.join(METADATA_FILE),
            format!("RDI1\nruntime/server.oci\nsha256:{}\n", "c".repeat(64)),
        )
        .unwrap();
        fs::set_permissions(
            installation_root.join(METADATA_FILE),
            fs::Permissions::from_mode(0o666),
        )
        .unwrap();
        assert!(matches!(
            resolver.resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap()),
            Err(StagedDockerImageResolverError::MetadataPermissions(0o666))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
