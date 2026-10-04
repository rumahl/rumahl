use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rumahl_core::{InstallationId, PackagePath};

use crate::DockerImageReference;
use crate::staged_docker_image_resolver::{FORMAT_VERSION, METADATA_FILE};

const INSTALLATION_DIRECTORY_MODE: u32 = 0o700;
const ROOT_DIRECTORY_MODE: u32 = 0o750;
const METADATA_MODE: u32 = 0o600;

#[derive(Debug, Clone)]
pub struct StagedDockerImageWriterConfig {
    root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedDockerImageWriterConfigError {
    RootMustBeAbsolute,
}

/// Publishes the immutable image reference consumed by [`StagedDockerImageResolver`].
///
/// The caller is responsible for loading and verifying the OCI image and for
/// obtaining a digest-pinned [`DockerImageReference`]. This writer only records
/// the handoff in the exact `RDI1` format the resolver requires, replacing any
/// previous reference atomically.
#[derive(Debug, Clone)]
pub struct StagedDockerImageWriter {
    config: StagedDockerImageWriterConfig,
}

#[derive(Debug)]
pub enum StagedDockerImageWriterError {
    CreateRoot(io::Error),
    RootIsNotDirectory,
    CreateInstallation(io::Error),
    InstallationIsNotDirectory,
    Metadata(io::Error),
    Write(io::Error),
    Commit(io::Error),
}

impl StagedDockerImageWriterConfig {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StagedDockerImageWriterConfigError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(StagedDockerImageWriterConfigError::RootMustBeAbsolute);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl StagedDockerImageWriter {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StagedDockerImageWriterConfigError> {
        Ok(Self {
            config: StagedDockerImageWriterConfig::new(root)?,
        })
    }

    pub fn config(&self) -> &StagedDockerImageWriterConfig {
        &self.config
    }

    pub fn write(
        &self,
        installation_id: &InstallationId,
        artifact: &PackagePath,
        image: &DockerImageReference,
    ) -> Result<(), StagedDockerImageWriterError> {
        prepare_directory(
            &self.config.root,
            ROOT_DIRECTORY_MODE,
            StagedDockerImageWriterError::CreateRoot,
            || StagedDockerImageWriterError::RootIsNotDirectory,
        )?;

        let installation_root = self.config.root.join(installation_id.to_string());
        prepare_directory(
            &installation_root,
            INSTALLATION_DIRECTORY_MODE,
            StagedDockerImageWriterError::CreateInstallation,
            || StagedDockerImageWriterError::InstallationIsNotDirectory,
        )?;

        let metadata = format!("{FORMAT_VERSION}\n{artifact}\n{image}\n");
        let temporary =
            installation_root.join(format!(".{METADATA_FILE}.tmp-{}", InstallationId::new()));

        {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(METADATA_MODE)
                .open(&temporary)
                .map_err(StagedDockerImageWriterError::Metadata)?;
            file.write_all(metadata.as_bytes())
                .map_err(StagedDockerImageWriterError::Write)?;
            file.sync_all()
                .map_err(StagedDockerImageWriterError::Write)?;
        }

        fs::rename(&temporary, installation_root.join(METADATA_FILE))
            .map_err(StagedDockerImageWriterError::Commit)
    }
}

fn prepare_directory<E>(
    path: &Path,
    mode: u32,
    create_error: impl Fn(io::Error) -> E + Copy,
    not_directory: impl FnOnce() -> E,
) -> Result<(), E> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(not_directory());
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(create_error)?;
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(create_error)?;
            Ok(())
        }
        Err(error) => Err(create_error(error)),
    }
}

impl fmt::Display for StagedDockerImageWriterConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMustBeAbsolute => write!(f, "staged image root path must be absolute"),
        }
    }
}

impl Error for StagedDockerImageWriterConfigError {}

impl fmt::Display for StagedDockerImageWriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateRoot(error) => write!(f, "staged image root could not be created: {error}"),
            Self::RootIsNotDirectory => {
                write!(f, "staged image root is not a non-symlink directory")
            }
            Self::CreateInstallation(error) => {
                write!(
                    f,
                    "staged installation directory could not be created: {error}"
                )
            }
            Self::InstallationIsNotDirectory => {
                write!(f, "staged installation path is not a non-symlink directory")
            }
            Self::Metadata(error) => {
                write!(f, "staged image metadata could not be opened: {error}")
            }
            Self::Write(error) => write!(f, "staged image metadata could not be written: {error}"),
            Self::Commit(error) => {
                write!(f, "staged image metadata could not be committed: {error}")
            }
        }
    }
}

impl Error for StagedDockerImageWriterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateRoot(error)
            | Self::CreateInstallation(error)
            | Self::Metadata(error)
            | Self::Write(error)
            | Self::Commit(error) => Some(error),
            Self::RootIsNotDirectory | Self::InstallationIsNotDirectory => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use rumahl_core::{AppId, AppIdentity, AppVersion, PublisherId, RuntimeDescriptor};

    use super::*;
    use crate::test_support::unique_test_root;
    use crate::{
        DockerImageResolver, RuntimeInstallationSpec, StagedDockerImageResolver,
        StagedDockerImageResolverConfig,
    };

    fn digest(byte: char) -> DockerImageReference {
        DockerImageReference::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
    }

    #[test]
    fn writes_metadata_the_resolver_accepts() {
        let root = unique_test_root('w');
        let installation_id = InstallationId::new();
        let artifact = PackagePath::parse("runtime/server.oci").unwrap();
        let image = digest('a');

        StagedDockerImageWriter::new(&root)
            .unwrap()
            .write(&installation_id, &artifact, &image)
            .unwrap();

        let resolver =
            StagedDockerImageResolver::new(StagedDockerImageResolverConfig::new(&root).unwrap());
        let spec = RuntimeInstallationSpec::new(
            AppIdentity::new(
                AppId::parse("com.rumahl.test").unwrap(),
                installation_id,
                PublisherId::parse("com.rumahl").unwrap(),
            ),
            AppVersion::new(1, 0, 0),
            RuntimeDescriptor::container(),
        );

        assert_eq!(resolver.resolve_image(&spec, &artifact).unwrap(), image);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_an_existing_reference_atomically() {
        let root = unique_test_root('w');
        let installation_id = InstallationId::new();
        let artifact = PackagePath::parse("runtime/server.oci").unwrap();
        let writer = StagedDockerImageWriter::new(&root).unwrap();

        writer
            .write(&installation_id, &artifact, &digest('a'))
            .unwrap();
        writer
            .write(&installation_id, &artifact, &digest('b'))
            .unwrap();

        let resolver =
            StagedDockerImageResolver::new(StagedDockerImageResolverConfig::new(&root).unwrap());
        let spec = RuntimeInstallationSpec::new(
            AppIdentity::new(
                AppId::parse("com.rumahl.test").unwrap(),
                installation_id,
                PublisherId::parse("com.rumahl").unwrap(),
            ),
            AppVersion::new(1, 0, 0),
            RuntimeDescriptor::container(),
        );

        assert_eq!(
            resolver.resolve_image(&spec, &artifact).unwrap().as_str(),
            digest('b').as_str()
        );

        let metadata =
            fs::symlink_metadata(root.join(installation_id.to_string()).join(METADATA_FILE))
                .unwrap();
        assert_eq!(metadata.mode() & 0o777, METADATA_MODE);
        fs::remove_dir_all(root).unwrap();
    }
}
