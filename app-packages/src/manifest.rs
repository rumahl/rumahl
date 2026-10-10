use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use rumahl_core::{
    AppId, AppManifest, AppSettingDeclaration, AppSettingKey, AppSettingKind, AppSettingOption,
    AppVersion, CapabilityId, ConnectorDeclaration, ConnectorTarget, PackagePath, PermissionId,
    PermissionRequest, PermissionScope, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
    RuntimeEntrypoint, RuntimeEntrypointId, RuntimeKind, RuntimeLifecycle,
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
    lifecycle: RuntimeLifecycle,
    permissions: Vec<PermissionRequest>,
    provided_capabilities: Vec<CapabilityId>,
    connectors: Vec<ConnectorDeclaration>,
    settings: Vec<AppSettingDeclaration>,
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
    InvalidSetting,
    InvalidLifecycle,
    InvalidPermission,
    InvalidCapability,
    InvalidConnector,
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
    #[serde(default)]
    lifecycle: Option<String>,
    #[serde(default)]
    permissions: Vec<RawPermission>,
    #[serde(default)]
    provided_capabilities: Vec<String>,
    #[serde(default)]
    connectors: Vec<RawConnector>,
    #[serde(default)]
    settings: Vec<RawSetting>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawConnector {
    target: String,
    entrypoint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawPermission {
    id: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSetting {
    key: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    options: Vec<RawSettingOption>,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSettingOption {
    value: String,
    label: String,
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
        self.app.to_app_manifest(&self.publisher_id)
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

    pub fn lifecycle(&self) -> RuntimeLifecycle {
        self.lifecycle
    }

    pub fn permissions(&self) -> &[PermissionRequest] {
        &self.permissions
    }

    pub fn provided_capabilities(&self) -> &[CapabilityId] {
        &self.provided_capabilities
    }

    pub fn connectors(&self) -> &[ConnectorDeclaration] {
        &self.connectors
    }

    pub fn settings(&self) -> &[AppSettingDeclaration] {
        &self.settings
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

        let lifecycle = match raw.lifecycle.as_deref() {
            None => RuntimeLifecycle::default(),
            Some(value) => {
                RuntimeLifecycle::parse(value).ok_or(PackageManifestError::InvalidLifecycle)?
            }
        };

        let mut permissions = Vec::with_capacity(raw.permissions.len());
        let mut permission_ids = HashSet::new();
        for permission in raw.permissions {
            let permission = parse_permission(permission)?;
            if !permission_ids.insert(permission.permission().clone()) {
                return Err(PackageManifestError::InvalidPermission);
            }
            permissions.push(permission);
        }

        let mut provided_capabilities = Vec::with_capacity(raw.provided_capabilities.len());
        let mut capability_ids = HashSet::new();
        for capability in raw.provided_capabilities {
            let capability = CapabilityId::parse(capability)
                .map_err(|_| PackageManifestError::InvalidCapability)?;
            if !capability_ids.insert(capability.clone()) {
                return Err(PackageManifestError::InvalidCapability);
            }
            provided_capabilities.push(capability);
        }

        let mut connectors = Vec::with_capacity(raw.connectors.len());
        for connector in raw.connectors {
            let target = ConnectorTarget::parse(connector.target)
                .map_err(|_| PackageManifestError::InvalidConnector)?;
            let entrypoint = RuntimeEntrypointId::parse(connector.entrypoint)
                .map_err(|_| PackageManifestError::InvalidConnector)?;
            connectors.push(ConnectorDeclaration::new(target, entrypoint));
        }

        let mut settings = Vec::with_capacity(raw.settings.len());
        let mut setting_keys = HashSet::new();
        for setting in raw.settings {
            let setting = parse_setting(setting)?;
            if !setting_keys.insert(setting.key().clone()) {
                return Err(PackageManifestError::InvalidSetting);
            }
            settings.push(setting);
        }

        Ok(Self {
            app_id,
            version,
            display_name,
            runtime,
            lifecycle,
            permissions,
            provided_capabilities,
            connectors,
            settings,
        })
    }

    fn to_app_manifest(
        &self,
        publisher_id: &PublisherId,
    ) -> Result<AppManifest, PackageManifestError> {
        let mut manifest = AppManifest::new(
            self.app_id.clone(),
            publisher_id.clone(),
            self.version,
            self.display_name.clone(),
            self.runtime.clone(),
        )
        .map_err(|_| PackageManifestError::InvalidAppManifest)?;
        manifest.set_lifecycle(self.lifecycle);
        for permission in &self.permissions {
            manifest
                .add_permission_request(permission.clone())
                .map_err(|_| PackageManifestError::InvalidPermission)?;
        }
        for capability in &self.provided_capabilities {
            manifest
                .add_provided_capability(capability.clone())
                .map_err(|_| PackageManifestError::InvalidCapability)?;
        }
        for connector in &self.connectors {
            manifest
                .add_connector(connector.clone())
                .map_err(|_| PackageManifestError::InvalidConnector)?;
        }
        for setting in &self.settings {
            manifest
                .add_setting(setting.clone())
                .map_err(|_| PackageManifestError::InvalidSetting)?;
        }
        Ok(manifest)
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

fn parse_setting(raw: RawSetting) -> Result<AppSettingDeclaration, PackageManifestError> {
    let key = AppSettingKey::parse(&raw.key).map_err(|_| PackageManifestError::InvalidSetting)?;

    let kind = match raw.kind.as_str() {
        "text" | "number" | "boolean" => {
            if !raw.options.is_empty() {
                return Err(PackageManifestError::InvalidSetting);
            }
            match raw.kind.as_str() {
                "text" => AppSettingKind::Text,
                "number" => AppSettingKind::Number,
                _ => AppSettingKind::Boolean,
            }
        }
        "select" => {
            if raw.options.is_empty() {
                return Err(PackageManifestError::InvalidSetting);
            }
            let mut options = Vec::with_capacity(raw.options.len());
            for option in raw.options {
                options.push(
                    AppSettingOption::new(option.value, option.label)
                        .map_err(|_| PackageManifestError::InvalidSetting)?,
                );
            }
            AppSettingKind::Select { options }
        }
        _ => return Err(PackageManifestError::InvalidSetting),
    };

    let mut declaration = AppSettingDeclaration::new(key, raw.title, kind)
        .map_err(|_| PackageManifestError::InvalidSetting)?;
    if let Some(description) = raw.description {
        declaration = declaration
            .with_description(description)
            .map_err(|_| PackageManifestError::InvalidSetting)?;
    }
    if raw.required {
        declaration = declaration.required();
    }
    Ok(declaration)
}

fn parse_permission(raw: RawPermission) -> Result<PermissionRequest, PackageManifestError> {
    let permission =
        PermissionId::parse(raw.id).map_err(|_| PackageManifestError::InvalidPermission)?;
    let scope = match raw.scope.as_deref() {
        None | Some("user-own") => PermissionScope::UserOwn,
        Some("app-private") => PermissionScope::AppPrivate,
        Some("user-selected") => PermissionScope::UserSelected,
        Some("explicit") => PermissionScope::Explicit,
        Some("family-shared") => PermissionScope::FamilyShared,
        Some("system") => PermissionScope::System,
        Some(_) => return Err(PackageManifestError::InvalidPermission),
    };
    let reason = raw
        .reason
        .map(|reason| reason.trim().to_owned())
        .filter(|reason| !reason.is_empty());
    if reason
        .as_ref()
        .is_some_and(|reason| reason.chars().count() > 240)
    {
        return Err(PackageManifestError::InvalidPermission);
    }
    Ok(PermissionRequest::new(
        permission,
        scope,
        raw.required,
        reason,
    ))
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
            Self::InvalidSetting => {
                write!(f, "package manifest declares an invalid app setting")
            }
            Self::InvalidLifecycle => {
                write!(f, "package manifest declares an unknown lifecycle")
            }
            Self::InvalidPermission => {
                write!(f, "package manifest declares an invalid permission")
            }
            Self::InvalidCapability => {
                write!(
                    f,
                    "package manifest declares an invalid provided capability"
                )
            }
            Self::InvalidConnector => {
                write!(f, "package manifest declares an invalid connector")
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
            | Self::InvalidAppManifest
            | Self::InvalidSetting
            | Self::InvalidLifecycle
            | Self::InvalidPermission
            | Self::InvalidCapability
            | Self::InvalidConnector => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with_settings(settings: &str) -> String {
        format!(
            r#"{{"formatVersion":1,"publisherId":"com.rumahl","app":{{"appId":"com.rumahl.notes","version":"1.0.0","displayName":"Notes","runtime":{{"kind":"web","entrypoints":[{{"id":"main","kind":"web-asset","path":"index.html"}}]}},"settings":{settings}}},"files":[]}}"#
        )
    }

    #[test]
    fn parses_typed_settings_into_the_app_manifest() {
        let bytes = manifest_with_settings(
            r#"[{"key":"server.url","title":"Server URL","description":"Backend endpoint","type":"text","required":true},{"key":"mode","title":"Mode","type":"select","options":[{"value":"fast","label":"Fast"},{"value":"safe","label":"Safe"}]},{"key":"retries","title":"Retries","type":"number"},{"key":"enabled","title":"Enabled","type":"boolean"}]"#,
        );
        let manifest = PackageManifest::from_bytes(bytes.as_bytes()).unwrap();
        assert_eq!(manifest.app().settings().len(), 4);

        let app = manifest.to_app_manifest().unwrap();
        let settings = app.settings();
        assert_eq!(settings[0].key().as_str(), "server.url");
        assert_eq!(settings[0].description(), Some("Backend endpoint"));
        assert!(settings[0].is_required());
        assert_eq!(settings[0].kind().as_str(), "text");
        assert_eq!(settings[1].kind().as_str(), "select");
        assert_eq!(settings[2].kind().as_str(), "number");
        assert_eq!(settings[3].kind().as_str(), "boolean");
    }

    #[test]
    fn rejects_unknown_setting_fields_and_bad_types() {
        let unknown =
            manifest_with_settings(r#"[{"key":"a","title":"A","type":"text","unexpected":true}]"#);
        assert!(PackageManifest::from_bytes(unknown.as_bytes()).is_err());

        let bad_type = manifest_with_settings(r#"[{"key":"a","title":"A","type":"date"}]"#);
        assert!(PackageManifest::from_bytes(bad_type.as_bytes()).is_err());

        let select_without_options =
            manifest_with_settings(r#"[{"key":"a","title":"A","type":"select"}]"#);
        assert!(PackageManifest::from_bytes(select_without_options.as_bytes()).is_err());

        let text_with_options = manifest_with_settings(
            r#"[{"key":"a","title":"A","type":"text","options":[{"value":"x","label":"X"}]}]"#,
        );
        assert!(PackageManifest::from_bytes(text_with_options.as_bytes()).is_err());
    }

    #[test]
    fn settings_stay_optional_for_existing_packages() {
        let bytes = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.notes","version":"1.0.0","displayName":"Notes","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]}},"files":[]}"#;
        let manifest = PackageManifest::from_bytes(bytes.as_bytes()).unwrap();
        assert!(manifest.app().settings().is_empty());
        assert!(manifest.to_app_manifest().unwrap().settings().is_empty());
    }

    #[test]
    fn parses_lifecycle_and_permissions_into_the_app_manifest() {
        let bytes = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.cloud","version":"1.0.0","displayName":"Cloud","runtime":{"kind":"container","entrypoints":[{"id":"main","kind":"container-artifact","path":"image.tar"}]},"lifecycle":"always-on","permissions":[{"id":"com.rumahl.os.window","scope":"user-own","required":true,"reason":"Control its window"}]},"files":[]}"#;
        let manifest = PackageManifest::from_bytes(bytes.as_bytes()).unwrap();
        assert_eq!(manifest.app().lifecycle(), RuntimeLifecycle::AlwaysOn);
        let app = manifest.to_app_manifest().unwrap();
        assert_eq!(app.lifecycle(), RuntimeLifecycle::AlwaysOn);
        assert_eq!(app.permission_requests().len(), 1);
        assert_eq!(
            app.permission_requests()[0].permission().as_str(),
            "com.rumahl.os.window"
        );
    }

    #[test]
    fn lifecycle_and_permissions_default_and_fail_closed() {
        let default_bytes = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.notes","version":"1.0.0","displayName":"Notes","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]}},"files":[]}"#;
        let app = PackageManifest::from_bytes(default_bytes.as_bytes())
            .unwrap()
            .to_app_manifest()
            .unwrap();
        assert_eq!(app.lifecycle(), RuntimeLifecycle::OnDemand);
        assert!(app.permission_requests().is_empty());

        let bad_lifecycle = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.notes","version":"1.0.0","displayName":"Notes","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]},"lifecycle":"sometimes"},"files":[]}"#;
        assert!(PackageManifest::from_bytes(bad_lifecycle.as_bytes()).is_err());

        let bad_permission = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.notes","version":"1.0.0","displayName":"Notes","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]},"permissions":[{"id":"nope"}]},"files":[]}"#;
        assert!(PackageManifest::from_bytes(bad_permission.as_bytes()).is_err());
    }

    #[test]
    fn parses_connectors_into_the_app_manifest() {
        let bytes = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.bridge","version":"1.0.0","displayName":"Bridge","runtime":{"kind":"container","entrypoints":[{"id":"main","kind":"container-artifact","path":"image.tar"}]},"connectors":[{"target":"nextcloud","entrypoint":"main"}]},"files":[]}"#;
        let app = PackageManifest::from_bytes(bytes.as_bytes())
            .unwrap()
            .to_app_manifest()
            .unwrap();
        assert_eq!(app.connectors().len(), 1);
        assert_eq!(app.connectors()[0].target().as_str(), "nextcloud");
        assert_eq!(app.connectors()[0].entrypoint().as_str(), "main");

        let bad_target = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.bridge","version":"1.0.0","displayName":"Bridge","runtime":{"kind":"container","entrypoints":[{"id":"main","kind":"container-artifact","path":"image.tar"}]},"connectors":[{"target":"Nextcloud","entrypoint":"main"}]},"files":[]}"#;
        assert!(PackageManifest::from_bytes(bad_target.as_bytes()).is_err());
    }

    #[test]
    fn parses_provided_capabilities_into_the_app_manifest() {
        let bytes = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.files","version":"1.0.0","displayName":"Files","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]},"providedCapabilities":["rumahl.files.preview","com.rumahl.files.search"]},"files":[]}"#;
        let app = PackageManifest::from_bytes(bytes.as_bytes())
            .unwrap()
            .to_app_manifest()
            .unwrap();
        assert_eq!(app.provided_capabilities().len(), 2);
        assert_eq!(
            app.provided_capabilities()[0].as_str(),
            "rumahl.files.preview"
        );

        let duplicate = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.files","version":"1.0.0","displayName":"Files","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]},"providedCapabilities":["rumahl.files.preview","rumahl.files.preview"]},"files":[]}"#;
        assert!(PackageManifest::from_bytes(duplicate.as_bytes()).is_err());

        let bad = r#"{"formatVersion":1,"publisherId":"com.rumahl","app":{"appId":"com.rumahl.files","version":"1.0.0","displayName":"Files","runtime":{"kind":"web","entrypoints":[{"id":"main","kind":"web-asset","path":"index.html"}]},"providedCapabilities":["nope"]},"files":[]}"#;
        assert!(PackageManifest::from_bytes(bad.as_bytes()).is_err());
    }
}
