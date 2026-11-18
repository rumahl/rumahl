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
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use rumahl_app_operations::{AppOperationRunner, AppRuntimeServices};
use rumahl_core::{
    AccountStateRepository, AppRuntimeInstallationState, AppRuntimeProvider, GrantAuthority,
    GrantIssuerPolicy, Identity, InMemoryGrantStore, InstallationId, InstalledApp, PermissionId,
    PermissionScope, PlatformRecovery, PlatformSnapshot, PlatformSnapshotRepository, PlatformState,
    ResourceRef, RuntimeEntrypointId, RuntimeKind, UserIdentity, UserRole,
};
use rumahl_oidc_provider::InstalledAppOriginResolver;
use rumahl_persistence_sqlite::{
    SecretEncryptionKeyId, SqliteAccountStateRepository, SqliteAppDatabaseProvider,
    SqliteAppOperationRepository, SqliteOidcClientRepository, SqliteSecretStore,
    SqliteSnapshotRepository,
};
use rumahl_platform_buildroot::{
    PackageImportError, PackageImporter, PackageImporterConfig, PackageImporterConfigError,
    Tpm2Authorization, Tpm2SealedKey, Tpm2UnsealKeyProvider, UnixAppRuntimeProvider,
    UnixAppRuntimeProviderConfig, UnixAppRuntimeProviderError, UnixDockerImageImporter,
    UnixDockerImageImporterConfig, UnixRuntimeSecretDelivery, UnixRuntimeSecretDeliveryConfig,
};

/// Routes app runtime operations: container apps use the supervisor, static web
/// apps have no external runtime and are accepted as a no-op.
struct PlatformRuntimeProvider {
    supervisor: UnixAppRuntimeProvider,
    storage: Option<(PathBuf, u64)>,
    channel: Option<std::sync::Arc<crate::channel::ChannelTokens>>,
}

#[derive(Debug)]
pub enum PlatformRuntimeError {
    Supervisor(UnixAppRuntimeProviderError),
    /// The app's private data has reached its quota; the OS refuses to start
    /// it rather than letting it grow without bound.
    QuotaExceeded,
    /// The app's private data directory cannot be prepared for the container.
    AppData(std::io::Error),
}

impl fmt::Display for PlatformRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supervisor(error) => write!(f, "runtime supervisor failed: {error}"),
            Self::QuotaExceeded => write!(f, "the app's storage quota is exhausted"),
            Self::AppData(error) => write!(f, "app data directory is unavailable: {error}"),
        }
    }
}

impl Error for PlatformRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::QuotaExceeded => None,
            Self::AppData(error) => Some(error),
        }
    }
}

impl PlatformRuntimeProvider {
    fn new(supervisor: UnixAppRuntimeProvider) -> Self {
        Self {
            supervisor,
            storage: None,
            channel: None,
        }
    }

    /// Enforces the per-app storage quota before a container app starts.
    fn with_storage_quota(mut self, root: PathBuf, quota: u64) -> Self {
        self.storage = Some((root, quota));
        self
    }

    /// Publishes the per-installation channel token into the app's data
    /// directory before the container starts.
    fn with_channel(mut self, channel: std::sync::Arc<crate::channel::ChannelTokens>) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Creates the private data directory (traversable by the supervisor's
    /// group) and hands the app its channel descriptor. The supervisor mounts
    /// the directory at `/data`, so both steps must happen before prepare.
    fn prepare_app_storage(&self, app: &InstalledApp) -> Result<(), PlatformRuntimeError> {
        let Some((root, _)) = self.storage.as_ref() else {
            return Ok(());
        };
        let identity = app.identity();
        let directory = root.join(identity.app_id().as_str());
        crate::channel::create_private_directory(&directory, 0o750)
            .map_err(PlatformRuntimeError::AppData)?;
        if let Some(channel) = self.channel.as_ref() {
            let installation = *identity.installation_id();
            match channel.issue(installation) {
                Ok(token) => {
                    if let Err(error) =
                        crate::channel::publish_app_material(&directory, installation, &token)
                    {
                        // The app still starts; without material the channel
                        // stays closed instead of becoming unauthenticated.
                        eprintln!("rumahl platform: app channel material unavailable: {error}");
                    }
                }
                Err(error) => {
                    eprintln!("rumahl platform: app channel token unavailable: {error}");
                }
            }
        }
        Ok(())
    }

    fn is_web(app: &InstalledApp) -> bool {
        app.manifest().runtime().kind() == RuntimeKind::Web
    }

    fn within_quota(&self, app: &InstalledApp) -> Result<(), PlatformRuntimeError> {
        let Some((root, quota)) = self.storage.as_ref() else {
            return Ok(());
        };
        match crate::apps::app_storage_usage_at(root, app.identity().app_id().as_str()) {
            // A missing directory is empty, not over quota.
            Ok(used) if used >= *quota => Err(PlatformRuntimeError::QuotaExceeded),
            _ => Ok(()),
        }
    }
}

impl AppRuntimeProvider for PlatformRuntimeProvider {
    type Error = PlatformRuntimeError;

    fn prepare_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        if Self::is_web(app) {
            return Ok(());
        }
        self.within_quota(app)?;
        self.prepare_app_storage(app)?;
        self.supervisor
            .prepare_installation(app)
            .map_err(PlatformRuntimeError::Supervisor)
    }

    fn activate_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        if Self::is_web(app) {
            return Ok(());
        }
        self.supervisor
            .activate_installation(app)
            .map_err(PlatformRuntimeError::Supervisor)
    }

    fn installation_state(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<AppRuntimeInstallationState, Self::Error> {
        self.supervisor
            .installation_state(installation_id)
            .map_err(PlatformRuntimeError::Supervisor)
    }

    fn deactivate_installation(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<bool, Self::Error> {
        self.supervisor
            .deactivate_installation(installation_id)
            .map_err(PlatformRuntimeError::Supervisor)
    }

    fn remove_installation(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<bool, Self::Error> {
        let removed = self
            .supervisor
            .remove_installation(installation_id)
            .map_err(PlatformRuntimeError::Supervisor)?;
        if let Some(channel) = self.channel.as_ref() {
            channel.revoke(installation_id);
        }
        Ok(removed)
    }
}

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

/// Explicit configuration for a package installation.
struct InstallConfig {
    runtime_uid: u32,
    control_socket: PathBuf,
    secret_socket: PathBuf,
    image_socket: PathBuf,
    trust_store: PathBuf,
    tpm_executable: PathBuf,
    secret_key_id: SecretEncryptionKeyId,
    secret_key_object: PathBuf,
    secret_key_policy_session: Option<PathBuf>,
    app_host_suffix: String,
}

impl InstallConfig {
    fn from_env() -> Result<Self, InstallError> {
        let secret_key_id = SecretEncryptionKeyId::parse(required("RUMAHL_SECRET_KEY_ID")?)
            .map_err(|_| InstallError::InvalidEnv("RUMAHL_SECRET_KEY_ID"))?;
        let secret_key_policy_session = match env::var("RUMAHL_SECRET_KEY_POLICY_SESSION") {
            Ok(path) if !path.is_empty() => Some(PathBuf::from(path)),
            _ => None,
        };
        Ok(Self {
            runtime_uid: required_u32("RUMAHL_RUNTIME_UID")?,
            control_socket: required_path("RUMAHL_RUNTIME_CONTROL_SOCKET")?,
            secret_socket: required_path("RUMAHL_RUNTIME_SECRET_SOCKET")?,
            image_socket: required_path("RUMAHL_RUNTIME_IMAGE_SOCKET")?,
            trust_store: required_path("RUMAHL_PACKAGE_TRUST_STORE")?,
            tpm_executable: required_path("RUMAHL_TPM2_EXECUTABLE")?,
            secret_key_id,
            secret_key_object: required_path("RUMAHL_SECRET_KEY_OBJECT")?,
            secret_key_policy_session,
            app_host_suffix: app_host_suffix()?,
        })
    }
}

/// The runtime provider with app storage wiring: container apps get their
/// private data directory mounted, and the per-app quota is enforced before an
/// app starts.
fn runtime_provider(
    config: &InstallConfig,
    state_dir: &Path,
) -> Result<PlatformRuntimeProvider, InstallError> {
    let mut provider_config =
        UnixAppRuntimeProviderConfig::new(&config.control_socket, config.runtime_uid)
            .map_err(InstallError::provider)?;
    let data_root = crate::apps_data_root();
    if let Some(root) = data_root.as_ref() {
        provider_config = provider_config
            .with_data_root(root)
            .map_err(InstallError::provider)?;
    }
    let mut provider = PlatformRuntimeProvider::new(UnixAppRuntimeProvider::new(provider_config));
    if let Some(root) = data_root {
        let quota = crate::apps::storage_quota_from_env().map_err(InstallError::Io)?;
        provider = provider.with_storage_quota(root, quota);
    }
    // The channel is independent of app storage: a container without `/data`
    // simply has nowhere to receive its descriptor and stays disconnected.
    provider = provider.with_channel(std::sync::Arc::new(
        crate::channel::ChannelTokens::in_state(state_dir),
    ));
    Ok(provider)
}

/// Builds the runtime adapter registry for the serve path when the supervisor
/// environment is configured. Returns `None` (fail closed) otherwise, so the
/// runtime control API answers 503 instead of assuming a runtime exists.
pub fn runtime_adapters_from_env(
    channels: &std::sync::Arc<rumahl_core::RuntimeChannelRegistry>,
    state_dir: &Path,
) -> Option<std::sync::Arc<rumahl_core::RuntimeAdapterRegistry>> {
    let config = InstallConfig::from_env().ok()?;
    let provider = std::sync::Arc::new(runtime_provider(&config, state_dir).ok()?);
    let mut registry = rumahl_core::RuntimeAdapterRegistry::new();
    registry
        .register(Box::new(
            rumahl_core::ProviderRuntimeAdapter::new(
                rumahl_core::RuntimeKind::Web,
                provider.clone(),
            )
            .with_channels(std::sync::Arc::clone(channels)),
        ))
        .ok()?;
    registry
        .register(Box::new(
            rumahl_core::ProviderRuntimeAdapter::new(rumahl_core::RuntimeKind::Container, provider)
                .with_channels(std::sync::Arc::clone(channels)),
        ))
        .ok()?;
    Some(std::sync::Arc::new(registry))
}

/// Verifies and installs a package, recovering interrupted container installs.
pub fn install_package(state_dir: &Path, package_dir: &Path) -> Result<(), InstallError> {
    install_with(&InstallConfig::from_env()?, state_dir, package_dir).map(|_| ())
}

/// Installs a package from a staged directory and returns its public metadata.
/// Uses the environment-configured supervisor/trust/TPM settings.
pub fn install_package_dir(
    state_dir: &Path,
    package_dir: &Path,
) -> Result<InstalledPackageInfo, InstallError> {
    install_with(&InstallConfig::from_env()?, state_dir, package_dir)
}

/// Public metadata of an installed package, read from the trusted snapshot.
#[derive(Debug, Clone)]
pub struct InstalledPackageInfo {
    pub app_id: String,
    pub installation_id: InstallationId,
    pub title: String,
    pub version: String,
}

/// Removes an installed app and every resource recorded in its operation plan.
/// Asset files and the private data directory are removed after the durable
/// uninstall commits, so a stale directory can never keep an app discoverable.
pub fn uninstall_package_dir(
    state_dir: &Path,
    installation_id: &InstallationId,
) -> Result<InstalledPackageInfo, InstallError> {
    uninstall_with(&InstallConfig::from_env()?, state_dir, installation_id)
}

fn uninstall_with(
    config: &InstallConfig,
    state_dir: &Path,
    installation_id: &InstallationId,
) -> Result<InstalledPackageInfo, InstallError> {
    let platform = state_dir.join("platform.sqlite");
    let origin = InstallationHostOriginResolver::new(config.app_host_suffix.clone())
        .map_err(InstallError::provider)?;
    let journal = SqliteAppOperationRepository::open(&platform).map_err(InstallError::provider)?;
    let databases = SqliteAppDatabaseProvider::open(state_dir.join("databases"))
        .map_err(InstallError::provider)?;
    let oidc_repository =
        SqliteOidcClientRepository::open(&platform).map_err(InstallError::provider)?;
    let runtime_provider = runtime_provider(config, state_dir)?;
    let secret_delivery = UnixRuntimeSecretDelivery::new(
        UnixRuntimeSecretDeliveryConfig::new(&config.secret_socket, config.runtime_uid)
            .map_err(InstallError::provider)?,
    );
    let key_provider = secret_key_provider(config)?;
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
    let app = state
        .installed_apps()
        .get_by_installation_id(installation_id)
        .cloned()
        .ok_or_else(|| InstallError::provider(std::io::Error::other("installation not found")))?;
    let info = InstalledPackageInfo {
        app_id: app.identity().app_id().as_str().to_owned(),
        installation_id: *installation_id,
        title: app.manifest().display_name().to_owned(),
        version: app.manifest().version().to_string(),
    };

    let runner = AppOperationRunner::new(
        journal,
        databases,
        AppRuntimeServices::new(runtime_provider, secret_delivery),
        oidc_repository,
        origin,
        snapshot_repository,
        secret_store,
    );
    runner
        .uninstall(installation_id, &mut state, &mut grants)
        .map_err(InstallError::provider)?;

    // These directories are outside the operation journal. The snapshot and
    // grants have already committed, so cleanup is best effort and cannot
    // resurrect the app.
    let assets = state_dir
        .join("app-assets")
        .join(installation_id.to_string());
    let _ = std::fs::remove_dir_all(assets);
    crate::app_runtime::remove_runtime_record(state_dir, installation_id);
    if let Some(apps_root) = crate::apps_data_root() {
        let _ = std::fs::remove_dir_all(apps_root.join(&info.app_id));
    }
    Ok(info)
}

fn install_with(
    config: &InstallConfig,
    state_dir: &Path,
    package_dir: &Path,
) -> Result<InstalledPackageInfo, InstallError> {
    let platform = state_dir.join("platform.sqlite");

    let origin = InstallationHostOriginResolver::new(config.app_host_suffix.clone())
        .map_err(InstallError::provider)?;
    let journal = SqliteAppOperationRepository::open(&platform).map_err(InstallError::provider)?;
    let databases = SqliteAppDatabaseProvider::open(state_dir.join("databases"))
        .map_err(InstallError::provider)?;
    let oidc_repository =
        SqliteOidcClientRepository::open(&platform).map_err(InstallError::provider)?;
    let runtime_provider = runtime_provider(config, state_dir)?;
    let secret_delivery = UnixRuntimeSecretDelivery::new(
        UnixRuntimeSecretDeliveryConfig::new(&config.secret_socket, config.runtime_uid)
            .map_err(InstallError::provider)?,
    );
    let image_importer = UnixDockerImageImporter::new(
        UnixDockerImageImporterConfig::new(&config.image_socket, config.runtime_uid)
            .map_err(InstallError::provider)?,
    );

    let key_provider = secret_key_provider(config)?;
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
        PackageImporterConfig::new(&config.trust_store, state_dir.join("app-assets"))
            .map_err(InstallError::Importer)?,
        image_importer,
    )
    .map_err(InstallError::Importer)?;

    let installed = importer
        .install(&runner, package_dir, &mut state, &mut grants)
        .map_err(InstallError::Import)?;

    // Public metadata of the freshly installed app, read from the trusted snapshot.
    let info = installed_info(&platform, installed.installation_id())?;

    // Record what the supervisor captured about the staged image, so the
    // extended configuration view can report it. Container packages only;
    // a failed record write means a broken state directory and surfaces.
    crate::app_runtime::write_runtime_record(
        state_dir,
        installed.installation_id(),
        installed.published(),
    )?;

    // Give the app its own directory under the apps volume, owner-only.
    if let Some(apps_root) = crate::apps_data_root() {
        let directory = apps_root.join(&info.app_id);
        std::fs::create_dir_all(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o750))?;
    }

    // Access is granted, not an install side effect: every existing account gets
    // an explicit launch grant (so the app appears in the catalog) and an
    // unconditional grant per provided capability (so the capability can be
    // invoked; the providing app still enforces its own resource rules).
    if let Some(snapshot) = SqliteSnapshotRepository::open(&platform)
        .map_err(InstallError::provider)?
        .load()
        .map_err(InstallError::provider)?
    {
        let accounts = SqliteAccountStateRepository::open(state_dir.join("accounts.sqlite"))
            .map_err(InstallError::provider)?
            .load()
            .map_err(InstallError::provider)?;
        let mut recovered = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();
        PlatformRecovery::new()
            .recover(&snapshot, &mut recovered, &mut grants)
            .map_err(|error| InstallError::Recovery(Box::new(error)))?;

        let launch_permission = PermissionId::parse(crate::apps::APP_LAUNCH_PERMISSION)
            .map_err(InstallError::provider)?;
        let launch_resource = crate::apps::app_launch_resource(*installed.installation_id());
        let mut targets: Vec<(PermissionId, PermissionScope, Vec<ResourceRef>)> = vec![(
            launch_permission,
            PermissionScope::Explicit,
            vec![launch_resource],
        )];

        if let Some(app) = recovered
            .installed_apps()
            .apps()
            .iter()
            .find(|app| app.installation_id() == installed.installation_id())
        {
            for capability in app.manifest().provided_capabilities() {
                if let Ok(permission) = PermissionId::parse(capability.as_str()) {
                    targets.push((permission, PermissionScope::System, Vec::new()));
                }
            }
        }

        let mut policy = GrantIssuerPolicy::new();
        for account in accounts.accounts().accounts() {
            policy.set_user_role(*account.user_id(), UserRole::Owner);
        }
        let mut granted = false;
        for account in accounts.accounts().accounts() {
            let issuer: Identity = UserIdentity::new(*account.user_id()).into();
            let subject: Identity = UserIdentity::new(*account.user_id()).into();
            for (permission, scope, resources) in &targets {
                // An update reuses the installation's grants: only missing
                // ones are issued, so repeated imports cannot pile duplicates.
                let exists = grants.grants().iter().any(|grant| {
                    grant.subject() == &subject
                        && grant.permission() == permission
                        && grant.scope() == *scope
                        && grant.resources() == resources.as_slice()
                });
                if exists {
                    continue;
                }
                let grant = GrantAuthority::new()
                    .issue(
                        &policy,
                        issuer.clone(),
                        subject.clone(),
                        permission.clone(),
                        *scope,
                        resources.clone(),
                    )
                    .map_err(InstallError::provider)?;
                grants.insert(grant);
                granted = true;
            }
        }
        if granted {
            SqliteSnapshotRepository::open(&platform)
                .map_err(InstallError::provider)?
                .store(&PlatformSnapshot::capture(&recovered, &grants))
                .map_err(InstallError::provider)?;
        }
    }

    println!(
        "Installed or updated {} ({:?}).",
        installed.installation_id(),
        installed.published()
    );
    Ok(info)
}

/// Reads the public metadata of an installed app from the trusted snapshot.
fn installed_info(
    platform: &Path,
    installation: &InstallationId,
) -> Result<InstalledPackageInfo, InstallError> {
    let snapshot = SqliteSnapshotRepository::open(platform)
        .map_err(InstallError::provider)?
        .load()
        .map_err(InstallError::provider)?;
    let app = snapshot
        .as_ref()
        .and_then(|snapshot| {
            snapshot
                .installed_apps()
                .iter()
                .find(|app| app.installation_id() == installation)
        })
        .ok_or_else(|| {
            InstallError::provider(std::io::Error::other("installed app missing from snapshot"))
        })?;
    Ok(InstalledPackageInfo {
        app_id: app.identity().app_id().as_str().to_owned(),
        installation_id: *installation,
        title: app.manifest().display_name().to_owned(),
        version: app.manifest().version().to_string(),
    })
}

fn secret_key_provider(config: &InstallConfig) -> Result<Tpm2UnsealKeyProvider, InstallError> {
    let authorization = match &config.secret_key_policy_session {
        Some(path) => Tpm2Authorization::PolicySession(path.clone()),
        None => Tpm2Authorization::Passwordless,
    };
    let sealed_key = Tpm2SealedKey::new(
        config.secret_key_id.clone(),
        &config.secret_key_object,
        authorization,
    )
    .map_err(InstallError::provider)?;
    Tpm2UnsealKeyProvider::new(
        &config.tpm_executable,
        config.secret_key_id.clone(),
        [sealed_key],
    )
    .map_err(InstallError::provider)
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
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use rumahl_app_packages::{GeneratedKey, PackageManifest, trust_store_json};
    use rumahl_core::{
        AppId, AppManifest, AppManifestValidator, AppVersion, InstalledApp, PackagePath,
        PublisherId, RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint, RuntimeEntrypointId,
    };
    use rumahl_platform_buildroot::{
        DockerImageReference, DockerImageTarget, RuntimeControlTarget, RuntimeControlTargetOutcome,
        RuntimeInstallationSpec, StagedDockerImageWriter, StagedImage, StagedImageEnvironment,
        UnixDockerImageServer, UnixDockerImageServerConfig, UnixRuntimeControlServer,
        UnixRuntimeControlServerConfig,
    };
    use serde_json::json;

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

    fn container_app() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.cloud").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Cloud",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    fn runtime_provider() -> PlatformRuntimeProvider {
        let config = UnixAppRuntimeProviderConfig::new("/run/rumahl/control.sock", 0).unwrap();
        PlatformRuntimeProvider::new(UnixAppRuntimeProvider::new(config))
    }

    #[test]
    fn static_web_apps_need_no_supervisor_runtime() {
        let provider = runtime_provider();
        let app = installed_app();

        provider.prepare_installation(&app).unwrap();
        provider.activate_installation(&app).unwrap();
    }

    #[test]
    fn container_apps_are_delegated_to_the_supervisor() {
        let provider = runtime_provider();
        let app = container_app();

        assert!(matches!(
            provider.prepare_installation(&app),
            Err(PlatformRuntimeError::Supervisor(_))
        ));
    }

    #[test]
    fn prepares_the_data_directory_and_channel_material_before_start() {
        use rumahl_platform_web::ChannelAuthenticator;
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let apps = temp.path().join("apps");
        let state = temp.path().join("state");
        std::fs::create_dir(&apps).unwrap();
        std::fs::create_dir(&state).unwrap();
        let provider = PlatformRuntimeProvider::new(UnixAppRuntimeProvider::new(
            UnixAppRuntimeProviderConfig::new("/run/rumahl/control.sock", 0).unwrap(),
        ))
        .with_storage_quota(apps.clone(), u64::MAX)
        .with_channel(std::sync::Arc::new(
            crate::channel::ChannelTokens::in_state(&state),
        ));
        let app = container_app();
        let app_id = app.identity().app_id().as_str().to_owned();
        let installation = *app.identity().installation_id();

        provider.prepare_app_storage(&app).unwrap();

        let directory = apps.join(&app_id);
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o750
        );
        let material = std::fs::read_to_string(directory.join(".rumahl/channel.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&material).unwrap();
        assert_eq!(value["protocol"], crate::channel::CHANNEL_PROTOCOL);
        assert_eq!(value["installationId"], installation.to_string());
        assert_eq!(value["socket"], crate::channel::CONTAINER_SOCKET);
        // The same token the listener will accept, and it is never world-readable.
        let auth = crate::channel::ChannelTokens::in_state(&state);
        assert_eq!(
            auth.authenticate(&installation.to_string(), value["token"].as_str().unwrap()),
            Some(installation)
        );
        assert_eq!(
            std::fs::metadata(directory.join(".rumahl/channel.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        // A repeated prepare reuses the token instead of rotating it under a
        // running app.
        provider.prepare_app_storage(&app).unwrap();
        let again: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(".rumahl/channel.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(again["token"], value["token"]);
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

    #[test]
    fn refuses_to_start_an_app_whose_data_is_over_quota() {
        let storage = tempfile::tempdir().unwrap();
        let app = container_app();
        let data = storage.path().join(app.identity().app_id().as_str());
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("big.bin"), vec![0_u8; 64]).unwrap();
        let config = UnixAppRuntimeProviderConfig::new("/run/rumahl/control.sock", 0).unwrap();
        let provider = PlatformRuntimeProvider::new(UnixAppRuntimeProvider::new(config))
            .with_storage_quota(storage.path().to_path_buf(), 16);

        let error = provider.prepare_installation(&app).unwrap_err();

        assert!(matches!(error, PlatformRuntimeError::QuotaExceeded));
        // Web apps own no container storage and are never blocked by it.
        let web = PlatformRuntimeProvider::new(UnixAppRuntimeProvider::new(
            UnixAppRuntimeProviderConfig::new("/run/rumahl/control.sock", 0).unwrap(),
        ))
        .with_storage_quota(storage.path().to_path_buf(), 16);
        assert!(web.prepare_installation(&installed_app()).is_ok());
    }

    #[test]
    fn installs_a_signed_web_package_end_to_end() {
        let state = tempfile::tempdir().unwrap();
        let package = tempfile::tempdir().unwrap();

        // A key pair plus the trust store that authorizes it.
        let generated = GeneratedKey::generate("publisher-1").unwrap();
        let trust_store_path = state.path().join("trust-store.json");
        fs::write(
            &trust_store_path,
            serde_json::to_vec(&trust_store_json(
                "publisher-1",
                "com.rumahl",
                &generated.public_key(),
            ))
            .unwrap(),
        )
        .unwrap();

        // Payload and the signed manifest.
        fs::create_dir_all(package.path().join("frontend")).unwrap();
        fs::write(
            package.path().join("frontend/index.html"),
            b"<html>notes</html>",
        )
        .unwrap();
        let template = json!({
            "formatVersion": 1,
            "publisherId": "com.rumahl",
            "app": {
                "appId": "com.rumahl.notes",
                "version": "1.0.0",
                "displayName": "Notes",
                "runtime": {
                    "kind": "web",
                    "entrypoints": [
                        { "id": "main", "kind": "web-asset", "path": "frontend/index.html" },
                    ],
                },
            },
        });
        let manifest =
            PackageManifest::from_bytes(&serde_json::to_vec(&template).unwrap()).unwrap();
        generated
            .key_store()
            .signer()
            .unwrap()
            .sign(package.path(), &manifest)
            .unwrap();

        let config = InstallConfig {
            runtime_uid: 0,
            control_socket: state.path().join("control.sock"),
            secret_socket: state.path().join("secret.sock"),
            image_socket: state.path().join("image.sock"),
            trust_store: trust_store_path,
            tpm_executable: PathBuf::from("/usr/bin/tpm2_unseal"),
            secret_key_id: SecretEncryptionKeyId::parse("root-1").unwrap(),
            secret_key_object: state.path().join("root-1.ctx"),
            secret_key_policy_session: None,
            app_host_suffix: "apps.rumahl.test".to_owned(),
        };

        install_with(&config, state.path(), package.path()).unwrap();

        // The snapshot records the app and its assets are published.
        let snapshot = SqliteSnapshotRepository::open(state.path().join("platform.sqlite"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.installed_apps().len(), 1);

        let installation = fs::read_dir(state.path().join("app-assets"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.is_dir())
            .unwrap();
        assert_eq!(
            fs::read(installation.join("frontend/index.html")).unwrap(),
            b"<html>notes</html>"
        );
    }

    #[test]
    fn installation_grants_launch_and_capabilities_to_existing_accounts() {
        let state = tempfile::tempdir().unwrap();
        let package = tempfile::tempdir().unwrap();

        let generated = GeneratedKey::generate("publisher-1").unwrap();
        let trust_store_path = state.path().join("trust-store.json");
        fs::write(
            &trust_store_path,
            serde_json::to_vec(&trust_store_json(
                "publisher-1",
                "com.rumahl",
                &generated.public_key(),
            ))
            .unwrap(),
        )
        .unwrap();
        fs::create_dir_all(package.path().join("frontend")).unwrap();
        fs::write(
            package.path().join("frontend/index.html"),
            b"<html>notes</html>",
        )
        .unwrap();
        let template = json!({
            "formatVersion": 1,
            "publisherId": "com.rumahl",
            "app": {
                "appId": "com.rumahl.notes",
                "version": "1.0.0",
                "displayName": "Notes",
                "runtime": {
                    "kind": "web",
                    "entrypoints": [
                        { "id": "main", "kind": "web-asset", "path": "frontend/index.html" },
                    ],
                },
                "providedCapabilities": ["rumahl.files.preview"],
            },
        });
        let manifest =
            PackageManifest::from_bytes(&serde_json::to_vec(&template).unwrap()).unwrap();
        generated
            .key_store()
            .signer()
            .unwrap()
            .sign(package.path(), &manifest)
            .unwrap();

        let password = rumahl_account_auth::SessionToken::generate()
            .unwrap()
            .encode()
            .to_string();
        crate::provision(
            &state.path().join("accounts.sqlite"),
            "developer",
            "Developer",
            password,
            crate::LocalPasswordBlocklist::default(),
        )
        .unwrap();

        let config = InstallConfig {
            runtime_uid: 0,
            control_socket: state.path().join("control.sock"),
            secret_socket: state.path().join("secret.sock"),
            image_socket: state.path().join("image.sock"),
            trust_store: trust_store_path,
            tpm_executable: PathBuf::from("/usr/bin/tpm2_unseal"),
            secret_key_id: SecretEncryptionKeyId::parse("root-1").unwrap(),
            secret_key_object: state.path().join("root-1.ctx"),
            secret_key_policy_session: None,
            app_host_suffix: "apps.rumahl.test".to_owned(),
        };
        install_with(&config, state.path(), package.path()).unwrap();

        let snapshot = SqliteSnapshotRepository::open(state.path().join("platform.sqlite"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        let accounts = SqliteAccountStateRepository::open(state.path().join("accounts.sqlite"))
            .unwrap()
            .load()
            .unwrap();
        let user = *accounts.accounts().accounts()[0].user_id();
        let identity = rumahl_platform_web::ShellIdentity {
            user_id: user,
            session_id: rumahl_core::SessionId::new(),
        };
        let authorized = crate::apps::authorized_apps(&snapshot, identity).unwrap();
        assert_eq!(authorized.len(), 1);
        assert_eq!(
            authorized[0].identity().app_id().as_str(),
            "com.rumahl.notes"
        );

        // The provided capability is granted unconditionally to the account.
        let capability = PermissionId::parse("rumahl.files.preview").unwrap();
        assert!(snapshot.grants().iter().any(|grant| {
            grant.permission() == &capability && grant.scope() == PermissionScope::System
        }));
    }

    /// Fake supervisor runtime target: preparation fails until the image
    /// reference has been staged, mirroring the Docker resolver's fail-closed
    /// behaviour.
    #[derive(Clone)]
    struct FakeRuntimeTarget {
        image_root: PathBuf,
    }

    impl RuntimeControlTarget for FakeRuntimeTarget {
        type Error = std::io::Error;

        fn prepare(
            &self,
            spec: RuntimeInstallationSpec,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            let installation = spec.identity().installation_id();
            let staged = self
                .image_root
                .join(installation.to_string())
                .join("image-reference")
                .is_file();
            Ok(if staged {
                RuntimeControlTargetOutcome::Accepted {
                    state: AppRuntimeInstallationState::Prepared,
                    changed: true,
                }
            } else {
                RuntimeControlTargetOutcome::Rejected
            })
        }

        fn activate(
            &self,
            _spec: RuntimeInstallationSpec,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Active,
                changed: true,
            })
        }

        fn installation_state(
            &self,
            _installation_id: rumahl_core::InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Active,
                changed: false,
            })
        }

        fn deactivate(
            &self,
            _installation_id: rumahl_core::InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Prepared,
                changed: false,
            })
        }

        fn remove(
            &self,
            _installation_id: rumahl_core::InstallationId,
        ) -> Result<RuntimeControlTargetOutcome, Self::Error> {
            Ok(RuntimeControlTargetOutcome::Accepted {
                state: AppRuntimeInstallationState::Absent,
                changed: false,
            })
        }
    }

    /// Fake supervisor image target: records the archive and publishes the RDI1
    /// reference the runtime target checks.
    #[derive(Clone)]
    struct FakeImageTarget {
        image_root: PathBuf,
        imported: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl DockerImageTarget for FakeImageTarget {
        type Error = std::io::Error;

        fn import_archive(
            &self,
            installation_id: &rumahl_core::InstallationId,
            artifact: &PackagePath,
            archive: &[u8],
        ) -> Result<StagedImage, Self::Error> {
            if archive.is_empty() {
                return Err(std::io::Error::other("empty archive"));
            }
            self.imported
                .lock()
                .unwrap()
                .push((installation_id.to_string(), artifact.as_str().to_owned()));
            let reference =
                DockerImageReference::parse(format!("sha256:{}", "a".repeat(64))).unwrap();
            StagedDockerImageWriter::new(&self.image_root)
                .unwrap()
                .write(installation_id, artifact, &reference)
                .unwrap();
            Ok(StagedImage::new(
                reference,
                vec![
                    StagedImageEnvironment::parse("PATH=/usr/bin:/bin").unwrap(),
                    StagedImageEnvironment::parse("RUMAHL_SEALED_SECRET=hidden").unwrap(),
                ],
            ))
        }
    }

    fn spawn_control_server(
        socket: &Path,
        uid: u32,
        target: FakeRuntimeTarget,
    ) -> thread::JoinHandle<()> {
        let server = UnixRuntimeControlServer::bind(
            UnixRuntimeControlServerConfig::new(socket, uid).unwrap(),
            target,
        )
        .unwrap();
        thread::spawn(move || while server.serve_once().is_ok() {})
    }

    fn spawn_image_server(
        socket: &Path,
        uid: u32,
        target: FakeImageTarget,
    ) -> thread::JoinHandle<()> {
        let server = UnixDockerImageServer::bind(
            UnixDockerImageServerConfig::new(socket, uid).unwrap(),
            target,
        )
        .unwrap();
        thread::spawn(move || while server.serve_once().is_ok() {})
    }

    #[test]
    fn installs_a_signed_container_package_end_to_end() {
        let state = tempfile::tempdir().unwrap();
        let package = tempfile::tempdir().unwrap();
        let image_root = state.path().join("images");
        fs::create_dir(&image_root).unwrap();
        let uid = fs::metadata(state.path()).unwrap().uid();

        let generated = GeneratedKey::generate("publisher-1").unwrap();
        let trust_store_path = state.path().join("trust-store.json");
        fs::write(
            &trust_store_path,
            serde_json::to_vec(&trust_store_json(
                "publisher-1",
                "com.rumahl",
                &generated.public_key(),
            ))
            .unwrap(),
        )
        .unwrap();

        fs::create_dir_all(package.path().join("runtime")).unwrap();
        fs::write(
            package.path().join("runtime/server.oci"),
            b"not-a-real-image",
        )
        .unwrap();
        let template = json!({
            "formatVersion": 1,
            "publisherId": "com.rumahl",
            "app": {
                "appId": "com.rumahl.cloud",
                "version": "1.0.0",
                "displayName": "Cloud",
                "runtime": {
                    "kind": "container",
                    "entrypoints": [
                        { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" },
                        { "id": "main", "kind": "endpoint", "endpoint": "web" },
                    ],
                },
            },
        });
        let manifest =
            PackageManifest::from_bytes(&serde_json::to_vec(&template).unwrap()).unwrap();
        generated
            .key_store()
            .signer()
            .unwrap()
            .sign(package.path(), &manifest)
            .unwrap();

        let control_socket = state.path().join("control.sock");
        let image_socket = state.path().join("image.sock");
        let imported = Arc::new(Mutex::new(Vec::new()));
        let _control = spawn_control_server(
            &control_socket,
            uid,
            FakeRuntimeTarget {
                image_root: image_root.clone(),
            },
        );
        let _images = spawn_image_server(
            &image_socket,
            uid,
            FakeImageTarget {
                image_root: image_root.clone(),
                imported: Arc::clone(&imported),
            },
        );

        let config = InstallConfig {
            runtime_uid: uid,
            control_socket,
            secret_socket: state.path().join("secret.sock"),
            image_socket,
            trust_store: trust_store_path,
            tpm_executable: PathBuf::from("/usr/bin/tpm2_unseal"),
            secret_key_id: SecretEncryptionKeyId::parse("root-1").unwrap(),
            secret_key_object: state.path().join("root-1.ctx"),
            secret_key_policy_session: None,
            app_host_suffix: "apps.rumahl.test".to_owned(),
        };

        install_with(&config, state.path(), package.path()).unwrap();

        let snapshot = SqliteSnapshotRepository::open(state.path().join("platform.sqlite"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.installed_apps().len(), 1);

        // The image was imported exactly once for the installed app.
        let installation_id = snapshot.installed_apps()[0].installation_id();
        assert_eq!(
            imported.lock().unwrap().as_slice(),
            &[(installation_id.to_string(), "runtime/server.oci".to_owned())]
        );

        // The supervisor's captured image metadata is recorded next to the
        // app state, with the credential-sounding entry still stored raw —
        // the display filter runs at read time, not at capture time.
        let record = crate::app_runtime::read_runtime_record(state.path(), installation_id)
            .expect("container install records its image metadata");
        assert_eq!(record.image, format!("sha256:{}", "a".repeat(64)));
        assert_eq!(
            record.environment,
            vec![
                ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                ("RUMAHL_SEALED_SECRET".to_owned(), "hidden".to_owned()),
            ]
        );

        // A newer package for the same app id updates the installation in
        // place: same identity, one app, and the image record follows.
        let upgrade = tempfile::tempdir().unwrap();
        fs::create_dir_all(upgrade.path().join("runtime")).unwrap();
        fs::write(
            upgrade.path().join("runtime/server.oci"),
            b"not-a-real-image",
        )
        .unwrap();
        let mut upgraded = template.clone();
        upgraded["app"]["version"] = json!("1.1.0");
        let manifest =
            PackageManifest::from_bytes(&serde_json::to_vec(&upgraded).unwrap()).unwrap();
        generated
            .key_store()
            .signer()
            .unwrap()
            .sign(upgrade.path(), &manifest)
            .unwrap();

        install_with(&config, state.path(), upgrade.path()).unwrap();

        let snapshot = SqliteSnapshotRepository::open(state.path().join("platform.sqlite"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.installed_apps().len(), 1);
        assert_eq!(
            snapshot.installed_apps()[0]
                .manifest()
                .version()
                .to_string(),
            "1.1.0"
        );
        assert_eq!(
            snapshot.installed_apps()[0].installation_id(),
            installation_id
        );
        assert_eq!(imported.lock().unwrap().len(), 2);
        assert!(
            crate::app_runtime::read_runtime_record(state.path(), installation_id).is_some(),
            "the update refreshes the recorded image metadata"
        );
    }
}
