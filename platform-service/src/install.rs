//! Verified package installation for the platform service.
//!
//! The platform service owns account/app state and the runtime-control channel,
//! but deliberately has no Docker access. Image import is delegated to the
//! Docker-capable runtime supervisor over an authenticated Unix socket.

use std::convert::Infallible;
use std::env;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use rumahl_app_operations::{AppOperationRunner, AppRuntimeServices};
use rumahl_core::{
    InMemoryGrantStore, InstalledApp, PlatformRecovery, PlatformSnapshotRepository, PlatformState,
    RuntimeEntrypointId,
};
use rumahl_oidc_provider::InstalledAppOriginResolver;
use rumahl_persistence_sqlite::{
    SecretEncryptionKeyId, SqliteAppDatabaseProvider, SqliteAppOperationRepository,
    SqliteOidcClientRepository, SqliteSecretStore, SqliteSnapshotRepository,
};
use rumahl_platform_buildroot::{
    PackageImportError, PackageImporter, PackageImporterConfig, PackageImporterConfigError,
    Tpm2Authorization, Tpm2SealedKey, Tpm2UnsealKeyProvider, UnixAppRuntimeProvider,
    UnixAppRuntimeProviderConfig, UnixDockerImageImporter, UnixDockerImageImporterConfig,
    UnixRuntimeSecretDelivery, UnixRuntimeSecretDeliveryConfig,
};

/// Derives an installed app's origin from its installation host.
pub struct InstallationHostOriginResolver {
    suffix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationHostOriginError {
    InvalidSuffix,
}

#[derive(Debug)]
pub enum InstallError {
    MissingEnv(&'static str),
    InvalidEnv(&'static str),
    Io(io::Error),
    Provider(Box<dyn Error + Send + Sync + 'static>),
    Importer(PackageImporterConfigError),
    Import(PackageImportError),
    Recovery(Box<dyn Error + Send + Sync + 'static>),
}

impl InstallError {
    fn provider(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Provider(Box::new(error))
    }
}

impl InstallationHostOriginResolver {
    pub fn new(suffix: impl Into<String>) -> Result<Self, InstallationHostOriginError> {
        let suffix = suffix.into();
        if !valid_host_suffix(&suffix) {
            return Err(InstallationHostOriginError::InvalidSuffix);
        }
        Ok(Self { suffix })
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }
}

impl InstalledAppOriginResolver for InstallationHostOriginResolver {
    type Error = Infallible;

    fn resolve_origin(
        &self,
        app: &InstalledApp,
        _entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error> {
        Ok(format!("https://{}.{}", app.installation_id(), self.suffix))
    }
}

/// Verifies and installs a package, recovering interrupted container installs.
pub fn install_package(state_dir: &Path, package_dir: &Path) -> Result<(), InstallError> {
    let platform = state_dir.join("platform.sqlite");
    let runtime_uid = required_u32("RUMAHL_RUNTIME_UID")?;
    let control_socket = required_path("RUMAHL_RUNTIME_CONTROL_SOCKET")?;
    let secret_socket = required_path("RUMAHL_RUNTIME_SECRET_SOCKET")?;
    let image_socket = required_path("RUMAHL_RUNTIME_IMAGE_SOCKET")?;
    let trust_store = required_path("RUMAHL_PACKAGE_TRUST_STORE")?;
    let suffix = app_host_suffix()?;

    let origin = InstallationHostOriginResolver::new(suffix).map_err(InstallError::provider)?;

    let journal = SqliteAppOperationRepository::open(&platform).map_err(InstallError::provider)?;
    let databases = SqliteAppDatabaseProvider::open(state_dir.join("databases"))
        .map_err(InstallError::provider)?;
    let oidc_repository =
        SqliteOidcClientRepository::open(&platform).map_err(InstallError::provider)?;
    let runtime_provider = UnixAppRuntimeProvider::new(
        UnixAppRuntimeProviderConfig::new(control_socket, runtime_uid)
            .map_err(InstallError::provider)?,
    );
    let secret_delivery = UnixRuntimeSecretDelivery::new(
        UnixRuntimeSecretDeliveryConfig::new(secret_socket, runtime_uid)
            .map_err(InstallError::provider)?,
    );
    let image_importer = UnixDockerImageImporter::new(
        UnixDockerImageImporterConfig::new(image_socket, runtime_uid)
            .map_err(InstallError::provider)?,
    );

    let key_provider = secret_key_provider()?;
    let secret_store =
        SqliteSecretStore::open(&platform, key_provider).map_err(InstallError::provider)?;

    let snapshot_repository =
        SqliteSnapshotRepository::open(&platform).map_err(InstallError::provider)?;
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    if let Some(snapshot) = snapshot_repository.load().map_err(InstallError::provider)? {
        PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .map_err(|error| InstallError::Recovery(Box::new(error)))?;
    }

    let runner = AppOperationRunner::new(
        journal,
        databases,
        AppRuntimeServices::new(runtime_provider, secret_delivery),
        oidc_repository,
        origin,
        snapshot_repository,
        secret_store,
    );

    let importer = PackageImporter::new(
        PackageImporterConfig::new(&trust_store, state_dir.join("app-assets"))
            .map_err(InstallError::Importer)?,
        image_importer,
    )
    .map_err(InstallError::Importer)?;

    let installed = importer
        .install(&runner, package_dir, &mut state, &mut grants)
        .map_err(InstallError::Import)?;

    println!(
        "Installed {} ({:?}).",
        installed.installation_id(),
        installed.published()
    );
    Ok(())
}

fn secret_key_provider() -> Result<Tpm2UnsealKeyProvider, InstallError> {
    let executable = required_path("RUMAHL_TPM2_EXECUTABLE")?;
    let key_id = SecretEncryptionKeyId::parse(required("RUMAHL_SECRET_KEY_ID")?)
        .map_err(|_| InstallError::InvalidEnv("RUMAHL_SECRET_KEY_ID"))?;
    let object_context = required_path("RUMAHL_SECRET_KEY_OBJECT")?;
    let authorization = match env::var("RUMAHL_SECRET_KEY_POLICY_SESSION") {
        Ok(path) if !path.is_empty() => Tpm2Authorization::PolicySession(PathBuf::from(path)),
        _ => Tpm2Authorization::Passwordless,
    };
    let sealed_key = Tpm2SealedKey::new(key_id.clone(), object_context, authorization)
        .map_err(InstallError::provider)?;
    Tpm2UnsealKeyProvider::new(executable, key_id, [sealed_key]).map_err(InstallError::provider)
}

fn app_host_suffix() -> Result<String, InstallError> {
    if let Ok(suffix) = env::var("RUMAHL_APP_HOST_SUFFIX") {
        return Ok(suffix);
    }
    let origin = required("RUMAHL_PUBLIC_ORIGIN")?;
    let host = origin
        .strip_prefix("https://")
        .ok_or(InstallError::InvalidEnv("RUMAHL_PUBLIC_ORIGIN"))?;
    let host = host.split(':').next().unwrap_or(host);
    Ok(format!("apps.{host}"))
}

fn required(name: &'static str) -> Result<String, InstallError> {
    env::var(name).map_err(|_| InstallError::MissingEnv(name))
}

fn required_path(name: &'static str) -> Result<PathBuf, InstallError> {
    Ok(PathBuf::from(required(name)?))
}

fn required_u32(name: &'static str) -> Result<u32, InstallError> {
    required(name)?
        .parse()
        .map_err(|_| InstallError::InvalidEnv(name))
}

fn valid_host_suffix(suffix: &str) -> bool {
    if suffix.is_empty() || suffix.len() > 253 || suffix.contains("..") {
        return false;
    }
    suffix.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    })
}

impl fmt::Display for InstallationHostOriginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSuffix => write!(f, "app host suffix is not a valid host name"),
        }
    }
}

impl Error for InstallationHostOriginError {}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv(name) => write!(f, "required environment variable {name} is missing"),
            Self::InvalidEnv(name) => {
                write!(f, "environment variable {name} has an invalid value")
            }
            Self::Io(error) => write!(f, "package installation I/O failed: {error}"),
            Self::Provider(_) => write!(f, "package installation provider setup failed"),
            Self::Importer(error) => write!(f, "package importer setup failed: {error}"),
            Self::Import(error) => write!(f, "package installation failed: {error}"),
            Self::Recovery(_) => write!(f, "platform snapshot recovery failed"),
        }
    }
}

impl Error for InstallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Provider(error) => Some(error.as_ref()),
            Self::Importer(error) => Some(error),
            Self::Import(error) => Some(error),
            Self::Recovery(error) => Some(error.as_ref()),
            Self::MissingEnv(_) | Self::InvalidEnv(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rumahl_core::{
        AppId, AppManifest, AppManifestValidator, AppVersion, InstalledApp, PackagePath,
        PublisherId, RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    use super::*;

    fn installed_app() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn validates_host_suffixes() {
        assert!(InstallationHostOriginResolver::new("apps.rumahl.home.arpa").is_ok());
        assert!(matches!(
            InstallationHostOriginResolver::new(""),
            Err(InstallationHostOriginError::InvalidSuffix)
        ));
        assert!(matches!(
            InstallationHostOriginResolver::new("apps..rumahl"),
            Err(InstallationHostOriginError::InvalidSuffix)
        ));
        assert!(matches!(
            InstallationHostOriginResolver::new("-apps.rumahl"),
            Err(InstallationHostOriginError::InvalidSuffix)
        ));
        assert!(matches!(
            InstallationHostOriginResolver::new("apps.rumahl:8443"),
            Err(InstallationHostOriginError::InvalidSuffix)
        ));
    }

    #[test]
    fn resolves_installation_origin() {
        let resolver = InstallationHostOriginResolver::new("apps.rumahl.test").unwrap();
        let app = installed_app();
        let origin = resolver
            .resolve_origin(&app, &RuntimeEntrypointId::parse("main").unwrap())
            .unwrap();

        assert_eq!(
            origin,
            format!("https://{}.apps.rumahl.test", app.installation_id())
        );
    }
}
