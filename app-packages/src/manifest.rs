use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use rumahl_core::{
    AppId, AppManifest, AppVersion, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
    RuntimeEntrypoint, RuntimeEntrypointId, RuntimeKind,
};
use serde::Deserialize;

use crate::digest::{parse_sha256_hex, to_hex};

/// File name of the signed package manifest inside a package root.
pub const MANIFEST_FILE: &str = "package.json";

/// Upper bound for the manifest document itself.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

/// Upper bound for the number of payload files a package may declare.
pub const MAX_FILES: usize = 2048;

/// Upper bound for any single payload file.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Upper bound for the summed size of all declared payload files.
pub const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;

const FORMAT_VERSION: u32 = 1;
const MAX_DISPLAY_NAME_CHARACTERS: usize = 120;

/// A validated application package manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    format_version: u32,
    publisher_id: PublisherId,
    app: PackageAppManifest,
    files: Vec<PackageFile>,
}

/// The application identity and runtime declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAppManifest {
    app_id: AppId,
    version: AppVersion,
    display_name: String,
    runtime: RuntimeDescriptor,
}

/// One payload file authenticated by the signed manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    path: PackagePath,
    size: u64,
    sha256: [u8; 32],
}

#[derive(Debug)]
pub enum PackageManifestError {
    TooLarge,
    Malformed(serde_json::Error),
    UnsupportedFormatVersion(u32),
    InvalidPublisherId,
    InvalidAppId,
    InvalidVersion,
    EmptyDisplayName,
    DisplayNameTooLong,
    InvalidRuntimeKind(String),
    DuplicateEntrypoint,
    InvalidRuntimeEntrypoint,
    InvalidEntrypointId,
    InvalidEntrypointKind(String),
    UnexpectedEntrypointField,
    InvalidEntrypointPath,
    InvalidEndpointId,
    InvalidFilePath,
    InvalidFileDigest,
    DuplicateFile,
    TooManyFiles,
    FileTooLarge,
    PackageTooLarge,
    InvalidAppManifest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawManifest {
    format_version: u32,
    publisher_id: String,
    app: RawApp,
    #[serde(default)]
    files: Vec<RawFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawApp {
    app_id: String,
    version: String,
    display_name: String,
    runtime: RawRuntime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawRuntime {
    kind: String,
    entrypoints: Vec<RawEntrypoint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawEntrypoint {
    id: String,
    kind: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawFile {
    path: String,
    size: u64,
    sha256: String,
}

impl PackageManifest {
    /// Parses and validates manifest bytes. The bytes must be the exact bytes a
    /// signature is verified against.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PackageManifestError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(PackageManifestError::TooLarge);
        }

        let raw: RawManifest =
            serde_json::from_slice(bytes).map_err(PackageManifestError::Malformed)?;
        Self::from_raw(raw)
    }

    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    pub fn publisher_id(&self) -> &PublisherId {
        &self.publisher_id
    }

    pub fn app(&self) -> &PackageAppManifest {
        &self.app
    }

    pub fn files(&self) -> &[PackageFile] {
        &self.files
    }

    /// Rebuilds the trusted core manifest used by the platform registration.
    pub fn to_app_manifest(&self) -> Result<AppManifest, PackageManifestError> {
        AppManifest::new(
            self.app.app_id.clone(),
            self.publisher_id.clone(),
            self.app.version,
            self.app.display_name.clone(),
            self.app.runtime.clone(),
        )
        .map_err(|_| PackageManifestError::InvalidAppManifest)
    }

    fn from_raw(raw: RawManifest) -> Result<Self, PackageManifestError> {
        if raw.format_version != FORMAT_VERSION {
            return Err(PackageManifestError::UnsupportedFormatVersion(
                raw.format_version,
            ));
        }

        let publisher_id = PublisherId::parse(raw.publisher_id)
            .map_err(|_| PackageManifestError::InvalidPublisherId)?;

        let app = PackageAppManifest::from_raw(raw.app)?;

        if raw.files.len() > MAX_FILES {
            return Err(PackageManifestError::TooManyFiles);
        }

        let mut paths = HashSet::new();
        let mut files = Vec::with_capacity(raw.files.len());
        let mut total_bytes = 0_u64;

        for file in raw.files {
            let path =
                PackagePath::parse(file.path).map_err(|_| PackageManifestError::InvalidFilePath)?;
            let sha256 =
                parse_sha256_hex(&file.sha256).ok_or(PackageManifestError::InvalidFileDigest)?;

            if file.size > MAX_FILE_BYTES {
                return Err(PackageManifestError::FileTooLarge);
            }

            total_bytes = total_bytes.saturating_add(file.size);
            if total_bytes > MAX_PACKAGE_BYTES {
                return Err(PackageManifestError::PackageTooLarge);
            }

            if !paths.insert(path.clone()) {
                return Err(PackageManifestError::DuplicateFile);
            }

            files.push(PackageFile {
                path,
                size: file.size,
                sha256,
            });
        }

        // Fail closed on a manifest that the core would reject, so a parsed
        // manifest is always installable in principle.
        app.to_app_manifest(&publisher_id)?;

        Ok(Self {
            format_version: raw.format_version,
            publisher_id,
            app,
            files,
        })
    }
}

impl PackageAppManifest {
    pub fn app_id(&self) -> &AppId {
        &self.app_id
    }

    pub fn version(&self) -> &AppVersion {
        &self.version
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn runtime(&self) -> &RuntimeDescriptor {
        &self.runtime
    }

    fn from_raw(raw: RawApp) -> Result<Self, PackageManifestError> {
        let app_id = AppId::parse(raw.app_id).map_err(|_| PackageManifestError::InvalidAppId)?;
        let version =
            AppVersion::parse(&raw.version).map_err(|_| PackageManifestError::InvalidVersion)?;

        let display_name = raw.display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err(PackageManifestError::EmptyDisplayName);
        }
        if display_name.chars().count() > MAX_DISPLAY_NAME_CHARACTERS {
            return Err(PackageManifestError::DisplayNameTooLong);
        }

        let runtime = parse_runtime(raw.runtime)?;

        Ok(Self {
            app_id,
            version,
            display_name,
            runtime,
        })
    }

    fn to_app_manifest(
        &self,
        publisher_id: &PublisherId,
    ) -> Result<AppManifest, PackageManifestError> {
        AppManifest::new(
            self.app_id.clone(),
            publisher_id.clone(),
            self.version,
            self.display_name.clone(),
            self.runtime.clone(),
        )
        .map_err(|_| PackageManifestError::InvalidAppManifest)
    }
}

impl PackageFile {
    pub fn path(&self) -> &PackagePath {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub fn sha256_hex(&self) -> String {
        to_hex(&self.sha256)
    }
}

fn parse_runtime(raw: RawRuntime) -> Result<RuntimeDescriptor, PackageManifestError> {
    let kind = parse_runtime_kind(&raw.kind)?;
    let mut descriptor = RuntimeDescriptor::new(kind);

    for entrypoint in raw.entrypoints {
        let entrypoint = parse_entrypoint(entrypoint)?;
        descriptor
            .add_entrypoint(entrypoint)
            .map_err(|error| match error {
                rumahl_core::RuntimeDescriptorError::DuplicateEntrypoint => {
                    PackageManifestError::DuplicateEntrypoint
                }
                rumahl_core::RuntimeDescriptorError::IncompatibleEntrypoint { .. } => {
                    PackageManifestError::InvalidRuntimeEntrypoint
                }
            })?;
    }

    Ok(descriptor)
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, PackageManifestError> {
    match value {
        "web" => Ok(RuntimeKind::Web),
        "container" => Ok(RuntimeKind::Container),
        "native" => Ok(RuntimeKind::Native),
        other => Err(PackageManifestError::InvalidRuntimeKind(other.to_owned())),
    }
}

fn parse_entrypoint(raw: RawEntrypoint) -> Result<RuntimeEntrypoint, PackageManifestError> {
    let id = RuntimeEntrypointId::parse(raw.id)
        .map_err(|_| PackageManifestError::InvalidEntrypointId)?;

    match raw.kind.as_str() {
        "web-asset" | "container-artifact" => {
            if raw.endpoint.is_some() {
                return Err(PackageManifestError::UnexpectedEntrypointField);
            }
            let path = raw
                .path
                .ok_or(PackageManifestError::InvalidEntrypointPath)?;
            let path = PackagePath::parse(path)
                .map_err(|_| PackageManifestError::InvalidEntrypointPath)?;
            Ok(if raw.kind == "web-asset" {
                RuntimeEntrypoint::web_asset(id, path)
            } else {
                RuntimeEntrypoint::container_artifact(id, path)
            })
        }
        "endpoint" => {
            if raw.path.is_some() {
                return Err(PackageManifestError::UnexpectedEntrypointField);
            }
            let endpoint = raw
                .endpoint
                .ok_or(PackageManifestError::InvalidEndpointId)?;
            let endpoint = RuntimeEndpointId::parse(endpoint)
                .map_err(|_| PackageManifestError::InvalidEndpointId)?;
            Ok(RuntimeEntrypoint::endpoint(id, endpoint))
        }
        other => Err(PackageManifestError::InvalidEntrypointKind(
            other.to_owned(),
        )),
    }
}

impl fmt::Display for PackageManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "package manifest exceeds the size limit"),
            Self::Malformed(error) => write!(f, "package manifest is malformed: {error}"),
            Self::UnsupportedFormatVersion(version) => {
                write!(f, "unsupported package manifest format version {version}")
            }
            Self::InvalidPublisherId => write!(f, "package manifest has an invalid publisher id"),
            Self::InvalidAppId => write!(f, "package manifest has an invalid app id"),
            Self::InvalidVersion => write!(f, "package manifest has an invalid app version"),
            Self::EmptyDisplayName => write!(f, "package manifest display name is empty"),
            Self::DisplayNameTooLong => write!(
                f,
                "package manifest display name exceeds {MAX_DISPLAY_NAME_CHARACTERS} characters"
            ),
            Self::InvalidRuntimeKind(kind) => {
                write!(f, "package manifest has an invalid runtime kind '{kind}'")
            }
            Self::DuplicateEntrypoint => {
                write!(f, "package manifest declares a duplicate entrypoint id")
            }
            Self::InvalidRuntimeEntrypoint => write!(
                f,
                "package manifest entrypoint is incompatible with its runtime"
            ),
            Self::InvalidEntrypointId => write!(f, "package manifest has an invalid entrypoint id"),
            Self::InvalidEntrypointKind(kind) => {
                write!(
                    f,
                    "package manifest has an invalid entrypoint kind '{kind}'"
                )
            }
            Self::UnexpectedEntrypointField => write!(
                f,
                "package manifest entrypoint sets a field unrelated to its kind"
            ),
            Self::InvalidEntrypointPath => {
                write!(f, "package manifest has an invalid entrypoint path")
            }
            Self::InvalidEndpointId => {
                write!(f, "package manifest has an invalid endpoint id")
            }
            Self::InvalidFilePath => write!(f, "package manifest has an invalid file path"),
            Self::InvalidFileDigest => {
                write!(f, "package manifest has an invalid file digest")
            }
            Self::DuplicateFile => write!(f, "package manifest declares a duplicate file path"),
            Self::TooManyFiles => {
                write!(f, "package manifest declares more than {MAX_FILES} files")
            }
            Self::FileTooLarge => write!(f, "package manifest declares an oversized file"),
            Self::PackageTooLarge => {
                write!(
                    f,
                    "package manifest declares more bytes than the package limit"
                )
            }
            Self::InvalidAppManifest => {
                write!(f, "package manifest cannot be converted to an app manifest")
            }
        }
    }
}

impl Error for PackageManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(error) => Some(error),
            Self::TooLarge
            | Self::UnsupportedFormatVersion(_)
            | Self::InvalidPublisherId
            | Self::InvalidAppId
            | Self::InvalidVersion
            | Self::EmptyDisplayName
            | Self::DisplayNameTooLong
            | Self::InvalidRuntimeKind(_)
            | Self::DuplicateEntrypoint
            | Self::InvalidRuntimeEntrypoint
            | Self::InvalidEntrypointId
            | Self::InvalidEntrypointKind(_)
            | Self::UnexpectedEntrypointField
            | Self::InvalidEntrypointPath
            | Self::InvalidEndpointId
            | Self::InvalidFilePath
            | Self::InvalidFileDigest
            | Self::DuplicateFile
            | Self::TooManyFiles
            | Self::FileTooLarge
            | Self::PackageTooLarge
            | Self::InvalidAppManifest => None,
        }
    }
}
