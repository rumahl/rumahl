//! Authorized installed-app catalog and short-lived, installation-bound asset leases.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::ShellIdentity;
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode},
    response::Response,
};
use rand::RngCore;
use rumahl_core::{AppId, InstallationId, PackagePath, SessionId};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogApp {
    pub id: String,
    pub installation_id: InstallationId,
    pub title: String,
    pub version: String,
    pub launchable: bool,
    /// Permission ids the installed app's manifest declares, exposed so the
    /// shell can gate the app <-> OS bridge per method.
    pub capabilities: Vec<String>,
    /// Runtime kind: `"web"`, `"native"` or `"container"`.
    pub runtime: String,
    /// Runtime lifecycle: `"always-on"` (service) or `"on-demand"`.
    pub lifecycle: String,
    /// External services this app ships a connector bundle for.
    pub connectors: Vec<String>,
}
/// One permission an app's manifest requests, typed for the shell's consent view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPermissionRequestInfo {
    pub permission: String,
    /// `user-selected`, `explicit`, `user-own`, `app-private`, `family-shared`
    /// or `system`.
    pub scope: String,
    pub required: bool,
    pub reason: Option<String>,
}
/// A live grant: what the app may do, on which resources, for how long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGrantInfo {
    pub id: String,
    pub permission: String,
    pub scope: String,
    pub resources: Vec<String>,
    /// `durable` (persisted, optionally with an expiry) or `session`
    /// (dies with the consenting browser session).
    pub validity: String,
    /// Unix seconds; `None` means "until revoked".
    pub expires_at: Option<u64>,
}
/// The consent state of one installed app as the calling user sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGrantState {
    pub requests: Vec<AppPermissionRequestInfo>,
    pub grants: Vec<AppGrantInfo>,
}
/// Storage accounting for one installed app's private data directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppStorageInfo {
    pub used_bytes: u64,
    /// The configured per-app quota; a write that would exceed it is refused
    /// by whichever OS component owns that write path.
    pub quota_bytes: u64,
}
/// The recorded runtime environment of an installed app, display-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRuntimeEnvironment {
    /// `web`, `container` or `native`.
    pub runtime: String,
    /// The immutable image reference of a container install; `None` when
    /// nothing was recorded (web/native apps, or an install from before the
    /// record existed).
    pub image: Option<String>,
    /// The image's own environment, without entries whose names could carry
    /// a credential.
    pub entries: Vec<AppRuntimeEnvEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRuntimeEnvEntry {
    pub name: String,
    pub value: String,
}
/// A user's consent decision, as submitted by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantIssue {
    pub permission: String,
    /// Personal-file resource keys the consent covers. Filesystem consent is
    /// always explicit: it names exactly what the app may touch.
    pub resources: Vec<String>,
    pub validity: GrantValidityChoice,
}
/// How long a consented grant stays usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantValidityChoice {
    /// Bound to the calling user's current browser session.
    Session,
    /// Persisted; `expires_in` bounds it in seconds, `None` keeps it until
    /// the user revokes it.
    Durable { expires_in: Option<u64> },
}
/// One app file access request, authorized per call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFileAccess {
    pub action: AppFileAction,
    /// Personal-file id of the calling user.
    pub key: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppFileAction {
    Read,
    Write { bytes: Vec<u8> },
}
/// The outcome of an app file access request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppFileOutcome {
    /// The grant covered this file; `name` is the stored file name.
    Allowed { name: String, size: u64 },
    /// The grant covered this file and the content was read.
    Content { name: String, bytes: Vec<u8> },
    /// No file with this id exists in the calling user's personal files.
    Missing,
    /// The authorization engine said no, with the reason the shell needs to
    /// decide between prompting for consent and reporting a hard denial.
    Denied(AppFileDeny),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppFileDeny {
    /// The app never asked for this permission; there is nothing to consent to.
    NoGrant,
    /// The app asked for this permission in its manifest but no grant covers
    /// the file yet — the shell should ask the user for consent.
    ApprovalRequired,
    /// A grant exists, but this specific file is outside its scope.
    OutsideScope,
}
/// One rendered setting declared by an app manifest, typed for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingInfo {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    /// `text`, `number`, `boolean` or `select`.
    pub kind: String,
    pub required: bool,
    pub options: Vec<AppSettingOptionInfo>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingOptionInfo {
    pub value: String,
    pub label: String,
}
/// The manifest-declared settings for one installed app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingsInfo {
    pub app: CatalogApp,
    pub manifest: Vec<AppSettingInfo>,
}
/// One entry of an app's private data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDataEntry {
    pub name: String,
    pub directory: bool,
    pub size: u64,
}
/// A read-only view of an app's private data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppData {
    Directory(Vec<AppDataEntry>),
    File {
        bytes: Vec<u8>,
        content_type: &'static str,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAccessError {
    Denied,
    Unavailable,
}
/// The runtime activation state of an installed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Running,
    Stopped,
}
impl RuntimeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}
/// A runtime control action requested for an installed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAction {
    Status,
    Start,
    Stop,
}
/// One file of a signed directory package, already base64-decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageUploadFile {
    pub path: String,
    pub bytes: Vec<u8>,
}
/// The public result of importing a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedApp {
    pub id: String,
    pub installation_id: InstallationId,
    pub title: String,
    pub version: String,
}
pub struct AppAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}
/// A resource reference supplied with a capability invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResource {
    pub namespace: String,
    pub kind: String,
    pub key: String,
}
/// The public result of a capability invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityOutcome {
    Invoked,
    Denied,
}
impl CapabilityOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invoked => "invoked",
            Self::Denied => "denied",
        }
    }
}
/// A web provider that must be reached through the shell hosting its iframe
/// (it has no live backend channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserDelivery {
    pub app_id: String,
    pub installation_id: String,
}
/// The outcome of a capability invocation plus the provider's result payload
/// (raw JSON text) when the provider returned one, or a browser delivery
/// instruction when the provider is a running web app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResult {
    pub outcome: CapabilityOutcome,
    pub result: Option<String>,
    pub browser: Option<BrowserDelivery>,
}
/// Every method must re-read installation, grants and live account/session state.
pub trait AppProvider: Send + Sync + 'static {
    fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError>;
    fn entrypoint(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<String, AppAccessError>;
    /// Resolves a Tier B surface declaration to its signed web entrypoint.
    /// Fails closed when the app has no such surface or it is data-only.
    fn surface_entrypoint(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _surface: &str,
    ) -> Result<String, AppAccessError> {
        Err(AppAccessError::Denied)
    }
    fn asset(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppAsset, AppAccessError>;
    /// Manifest-declared settings for an installed app, in declaration order.
    fn settings(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<Vec<AppSettingInfo>, AppAccessError>;
    /// Read-only listing or contents of an app's private data directory.
    fn data(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppData, AppAccessError>;
    /// Resolves an internal path (`/home/<caller>/...` or `rumahl://home/...`)
    /// to the caller's personal-store id for channel file access. Only the
    /// caller's own home is addressable; grants still apply to the id.
    fn resolve_channel_path(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _path: &str,
    ) -> Result<String, AppAccessError> {
        Err(AppAccessError::Denied)
    }
    /// Runtime activation state. Fails closed when no runtime control is wired.
    fn runtime_status(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Starts an app runtime (used for on-demand apps and manual control).
    fn start_runtime(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Stops an app runtime while retaining its prepared namespace.
    fn stop_runtime(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Removes an installation and releases its runtime, grants, secrets and data.
    fn uninstall_app(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<(), AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Verifies and installs a signed package uploaded from the shell. Fails
    /// closed when no importer is wired.
    fn import_package(
        &self,
        _identity: ShellIdentity,
        _files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Authorizes and delivers a capability invocation to the providing app.
    /// Fails closed when no dispatcher is wired.
    fn invoke_capability(
        &self,
        _identity: ShellIdentity,
        _capability: String,
        _resource: Option<CapabilityResource>,
    ) -> Result<CapabilityResult, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// The consent state of one installed app for the calling user: what the
    /// manifest requests and which grants currently apply.
    fn app_grants(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<AppGrantState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Issues a grant from an explicit user consent. The provider validates
    /// the request against the app's manifest and the OS permission catalog,
    /// binds session grants to the calling session and persists durable
    /// grants in the platform snapshot. Fails closed when no grant store is
    /// wired.
    fn issue_app_grant(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _issue: &GrantIssue,
    ) -> Result<AppGrantInfo, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Revokes a grant the calling user issued. Fails closed when no grant
    /// store is wired.
    fn revoke_app_grant(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _grant_id: &str,
    ) -> Result<(), AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Reads or writes one personal file of the calling user on an app's
    /// behalf, authorized per call against the app's grants. Fails closed
    /// when no personal-files store is wired.
    fn app_file(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// The recorded runtime environment of an installed app. Fails closed
    /// when no runtime record store is wired.
    fn app_runtime_environment(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<AppRuntimeEnvironment, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Measured storage usage of an installed app's data directory. Fails
    /// closed when no data root is wired.
    fn app_storage(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<AppStorageInfo, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// The browser session ended: memory-only grants issued for it must not
    /// outlive it. Best effort by design.
    fn session_ended(&self, _session: SessionId) {}
}
#[derive(Clone)]
struct Lease {
    identity: ShellIdentity,
    installation: InstallationId,
    /// `Some` for a Tier B surface lease; `None` for a normal window lease.
    surface: Option<String>,
    expires: Instant,
}
pub struct AppAccess {
    provider: Arc<dyn AppProvider>,
    shell_origin: String,
    suffix: String,
    port: Option<u16>,
    leases: Mutex<HashMap<String, Lease>>,
}
pub struct AppLaunch {
    pub app: CatalogApp,
    pub lease: String,
    pub frame_url: String,
}
pub struct SurfaceLaunch {
    pub app: CatalogApp,
    pub surface: String,
    pub lease: String,
    pub frame_url: String,
}
impl AppAccess {
    fn installed(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<CatalogApp, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        self.catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)
    }
    /// The consent state of one installed app: what its manifest requests and
    /// which grants the calling user has issued.
    pub fn app_grants(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<AppGrantState, AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider.app_grants(identity, app.installation_id)
    }
    /// Issues a grant from an explicit user consent.
    pub fn issue_app_grant(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        issue: &GrantIssue,
    ) -> Result<AppGrantInfo, AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider
            .issue_app_grant(identity, app.installation_id, issue)
    }
    /// Revokes one of the calling user's grants for an installed app.
    pub fn revoke_app_grant(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        grant_id: &str,
    ) -> Result<(), AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider
            .revoke_app_grant(identity, app.installation_id, grant_id)
    }
    /// Reads or writes one personal file on an app's behalf, authorized per
    /// call against the app's grants.
    pub fn app_file(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider
            .app_file(identity, app.installation_id, access)
    }
    /// The recorded runtime environment of an installed app.
    pub fn app_runtime_environment(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<AppRuntimeEnvironment, AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider
            .app_runtime_environment(identity, app.installation_id)
    }
    /// Measured storage usage of an installed app's data directory.
    pub fn app_storage(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<AppStorageInfo, AppAccessError> {
        let app = self.installed(identity, id, installation)?;
        self.provider.app_storage(identity, app.installation_id)
    }

    /// Drops the memory-only grants of an ended browser session.
    pub fn session_ended(&self, session: SessionId) {
        self.provider.session_ended(session);
    }

    /// Resolves an internal path for a channel app with the invocation's
    /// identity, so an app can use `/home/<user>/...` without ever naming
    /// another account or bypassing the grant on the resolved id.
    pub fn resolve_channel_path(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<String, AppAccessError> {
        self.catalog(identity)?
            .into_iter()
            .find(|app| app.installation_id == installation)
            .ok_or(AppAccessError::Denied)?;
        self.provider
            .resolve_channel_path(identity, installation, path)
    }
    /// File access for a channel app, addressed by installation id only: the
    /// identity comes from the user's in-flight capability invocation, never
    /// from the app itself.
    pub fn app_file_for_installation(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError> {
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.installation_id == installation)
            .ok_or(AppAccessError::Denied)?;
        self.provider
            .app_file(identity, app.installation_id, access)
    }
    pub fn new(
        provider: Arc<dyn AppProvider>,
        shell_origin: &Url,
        suffix: &str,
    ) -> Result<Self, AppAccessError> {
        let valid = suffix.len() <= 180
            && suffix.contains('.')
            && suffix.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            });
        if !valid || shell_origin.host_str() == Some(suffix) || shell_origin.scheme() != "https" {
            return Err(AppAccessError::Unavailable);
        }
        Ok(Self {
            provider,
            shell_origin: shell_origin.origin().ascii_serialization(),
            suffix: suffix.into(),
            port: shell_origin.port(),
            leases: Mutex::new(HashMap::new()),
        })
    }
    fn authority(&self, installation: InstallationId) -> String {
        format!(
            "{installation}.{}{}",
            self.suffix,
            self.port.map_or(String::new(), |port| format!(":{port}"))
        )
    }
    pub fn accepts_host(&self, authority: &str) -> bool {
        let Some((id, _)) = authority.split_once('.') else {
            return false;
        };
        InstallationId::parse(id)
            .is_ok_and(|installation| self.authority(installation) == authority)
    }
    pub fn frame_source(&self) -> String {
        format!(
            "https://*.{}{}",
            self.suffix,
            self.port.map_or(String::new(), |port| format!(":{port}"))
        )
    }
    pub fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
        let apps = self.provider.catalog(identity)?;
        if apps.len() > 256 {
            return Err(AppAccessError::Unavailable);
        }
        Ok(apps)
    }
    pub fn settings(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<AppSettingsInfo, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        let manifest = self.provider.settings(identity, app.installation_id)?;
        Ok(AppSettingsInfo { app, manifest })
    }
    pub fn data(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        path: &str,
    ) -> Result<AppData, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        self.provider.data(identity, app.installation_id, path)
    }
    pub fn uninstall(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<(), AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        self.provider.uninstall_app(identity, app.installation_id)
    }
    pub fn runtime(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        action: RuntimeAction,
    ) -> Result<RuntimeState, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        match action {
            RuntimeAction::Status => self.provider.runtime_status(identity, app.installation_id),
            RuntimeAction::Start => self.provider.start_runtime(identity, app.installation_id),
            RuntimeAction::Stop => self.provider.stop_runtime(identity, app.installation_id),
        }
    }
    pub fn import_package(
        &self,
        identity: ShellIdentity,
        files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        self.provider.import_package(identity, files)
    }
    pub fn invoke_capability(
        &self,
        identity: ShellIdentity,
        capability: String,
        resource: Option<CapabilityResource>,
    ) -> Result<CapabilityResult, AppAccessError> {
        self.provider
            .invoke_capability(identity, capability, resource)
    }
    pub fn launch(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        previous: Option<&str>,
    ) -> Result<AppLaunch, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| {
                app.id == id && app.installation_id.to_string() == installation && app.launchable
            })
            .ok_or(AppAccessError::Denied)?;
        let entry = self.provider.entrypoint(identity, app.installation_id)?;
        PackagePath::parse(&entry).map_err(|_| AppAccessError::Unavailable)?;
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| AppAccessError::Unavailable)?;
        let now = Instant::now();
        leases.retain(|_, lease| lease.expires > now);
        let token = if let Some(previous) = previous {
            leases
                .get(previous)
                .filter(|lease| {
                    lease.identity == identity
                        && lease.installation == app.installation_id
                        && lease.surface.is_none()
                })
                .ok_or(AppAccessError::Denied)?;
            previous.to_owned()
        } else {
            if leases.len() >= 1024
                || leases
                    .values()
                    .filter(|lease| lease.identity == identity)
                    .count()
                    >= 32
            {
                return Err(AppAccessError::Unavailable);
            }
            let mut random = [0_u8; 32];
            rand::rng().fill_bytes(&mut random);
            random.iter().map(|byte| format!("{byte:02x}")).collect()
        };
        leases.insert(
            token.clone(),
            Lease {
                identity,
                installation: app.installation_id,
                surface: None,
                expires: now + Duration::from_secs(90),
            },
        );
        let frame_url = format!(
            "https://{}/launch/{token}/{entry}",
            self.authority(app.installation_id)
        );
        Ok(AppLaunch {
            app,
            lease: token,
            frame_url,
        })
    }
    pub fn surface(
        &self,
        identity: ShellIdentity,
        installation: &str,
        surface_id: &str,
        previous: Option<&str>,
    ) -> Result<SurfaceLaunch, AppAccessError> {
        rumahl_core::ContributionId::parse(surface_id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.installation_id.to_string() == installation && app.launchable)
            .ok_or(AppAccessError::Denied)?;
        let entry = self
            .provider
            .surface_entrypoint(identity, app.installation_id, surface_id)?;
        PackagePath::parse(&entry).map_err(|_| AppAccessError::Unavailable)?;
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| AppAccessError::Unavailable)?;
        let now = Instant::now();
        leases.retain(|_, lease| lease.expires > now);
        let token = if let Some(previous) = previous {
            leases
                .get(previous)
                .filter(|lease| {
                    lease.identity == identity
                        && lease.installation == app.installation_id
                        && lease.surface.as_deref() == Some(surface_id)
                })
                .ok_or(AppAccessError::Denied)?;
            previous.to_owned()
        } else {
            if leases.len() >= 1024
                || leases
                    .values()
                    .filter(|lease| lease.identity == identity)
                    .count()
                    >= 32
            {
                return Err(AppAccessError::Unavailable);
            }
            let mut random = [0_u8; 32];
            rand::rng().fill_bytes(&mut random);
            random.iter().map(|byte| format!("{byte:02x}")).collect()
        };
        leases.insert(
            token.clone(),
            Lease {
                identity,
                installation: app.installation_id,
                surface: Some(surface_id.to_owned()),
                expires: now + Duration::from_secs(90),
            },
        );
        let frame_url = format!(
            "https://{}/launch/{token}/{entry}",
            self.authority(app.installation_id)
        );
        Ok(SurfaceLaunch {
            app,
            surface: surface_id.to_owned(),
            lease: token,
            frame_url,
        })
    }

    pub fn resource(&self, host: &str, path: &str) -> Result<AppAsset, AppAccessError> {
        let rest = path
            .strip_prefix("/launch/")
            .ok_or(AppAccessError::Denied)?;
        let (token, asset) = rest.split_once('/').ok_or(AppAccessError::Denied)?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppAccessError::Denied);
        }
        PackagePath::parse(asset).map_err(|_| AppAccessError::Denied)?;
        let lease = self
            .leases
            .lock()
            .map_err(|_| AppAccessError::Unavailable)?
            .get(token)
            .filter(|lease| lease.expires > Instant::now())
            .cloned()
            .ok_or(AppAccessError::Denied)?;
        if host != self.authority(lease.installation) {
            return Err(AppAccessError::Denied);
        }
        self.provider
            .asset(lease.identity, lease.installation, asset)
    }
    pub fn asset_response(&self, host: &str, asset: AppAsset) -> Response {
        let mut response = Response::new(Body::from(asset.bytes));
        let origin = format!("https://{host}");
        let csp = format!(
            "sandbox allow-scripts; default-src 'none'; script-src {origin} 'unsafe-inline'; style-src {origin} 'unsafe-inline'; img-src {origin} data:; font-src {origin}; connect-src {origin}; frame-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'; frame-ancestors {}",
            self.shell_origin
        );
        let headers = response.headers_mut();
        headers.insert("content-type", HeaderValue::from_static(asset.content_type));
        headers.insert(
            "content-security-policy",
            HeaderValue::from_str(&csp).expect("validated app origin"),
        );
        headers.insert(
            "cache-control",
            HeaderValue::from_static("private, no-store"),
        );
        headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
        headers.insert(
            "x-content-type-options",
            HeaderValue::from_static("nosniff"),
        );
        // Sandboxed ES-module requests have an opaque Origin; never allow credentials.
        headers.insert(
            "access-control-allow-origin",
            HeaderValue::from_static("null"),
        );
        headers.insert(
            "permissions-policy",
            HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
            ),
        );
        response
    }
}
pub fn app_error(error: AppAccessError) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = match error {
        AppAccessError::Denied => StatusCode::NOT_FOUND,
        AppAccessError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

impl crate::app_channel::ChannelFiles for AppAccess {
    fn access(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError> {
        self.app_file_for_installation(identity, installation, access)
    }

    fn resolve(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<String, AppAccessError> {
        self.resolve_channel_path(identity, installation, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{SessionId, UserId};
    struct Provider {
        owner: ShellIdentity,
        installation: InstallationId,
    }
    impl AppProvider for Provider {
        fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
            Ok(if identity == self.owner {
                vec![CatalogApp {
                    id: "com.rumahl.test".into(),
                    installation_id: self.installation,
                    title: "Test".into(),
                    version: "1.0.0".into(),
                    launchable: true,
                    capabilities: vec!["com.rumahl.os.window".into()],
                    runtime: "web".into(),
                    lifecycle: "on-demand".into(),
                    connectors: vec![],
                }]
            } else {
                vec![]
            })
        }
        fn runtime_status(
            &self,
            _: ShellIdentity,
            _: InstallationId,
        ) -> Result<RuntimeState, AppAccessError> {
            Ok(RuntimeState::Running)
        }
        fn uninstall_app(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
        ) -> Result<(), AppAccessError> {
            if identity == self.owner && installation == self.installation {
                Ok(())
            } else {
                Err(AppAccessError::Denied)
            }
        }
        fn import_package(
            &self,
            _: ShellIdentity,
            _: Vec<PackageUploadFile>,
        ) -> Result<ImportedApp, AppAccessError> {
            Ok(ImportedApp {
                id: "com.rumahl.test".into(),
                installation_id: self.installation,
                title: "Test".into(),
                version: "1.0.0".into(),
            })
        }
        fn entrypoint(
            &self,
            _: ShellIdentity,
            _: InstallationId,
        ) -> Result<String, AppAccessError> {
            Ok("web/index.html".into())
        }
        fn surface_entrypoint(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
            surface: &str,
        ) -> Result<String, AppAccessError> {
            if identity == self.owner
                && installation == self.installation
                && surface == "com.rumahl.test.desktop"
            {
                Ok("web/surface.html".into())
            } else {
                Err(AppAccessError::Denied)
            }
        }
        fn asset(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
            _: &str,
        ) -> Result<AppAsset, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(AppAsset {
                bytes: b"test".to_vec(),
                content_type: "text/html; charset=utf-8",
            })
        }
        fn settings(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
        ) -> Result<Vec<AppSettingInfo>, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(vec![AppSettingInfo {
                key: "server.url".into(),
                title: "Server URL".into(),
                description: Some("Backend endpoint".into()),
                kind: "text".into(),
                required: true,
                options: Vec::new(),
            }])
        }
        fn data(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
            _: &str,
        ) -> Result<AppData, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(AppData::Directory(vec![AppDataEntry {
                name: "notes.db".into(),
                directory: false,
                size: 12,
            }]))
        }
        fn invoke_capability(
            &self,
            identity: ShellIdentity,
            capability: String,
            _: Option<CapabilityResource>,
        ) -> Result<CapabilityResult, AppAccessError> {
            if identity != self.owner {
                return Err(AppAccessError::Denied);
            }
            Ok(match capability.as_str() {
                "com.rumahl.test.run" => CapabilityResult {
                    outcome: CapabilityOutcome::Invoked,
                    result: Some("{\"ran\":true}".into()),
                    browser: None,
                },
                "com.rumahl.test.browser" => CapabilityResult {
                    outcome: CapabilityOutcome::Invoked,
                    result: None,
                    browser: Some(BrowserDelivery {
                        app_id: "com.rumahl.test".into(),
                        installation_id: self.installation.to_string(),
                    }),
                },
                _ => CapabilityResult {
                    outcome: CapabilityOutcome::Denied,
                    result: None,
                    browser: None,
                },
            })
        }
    }
    #[test]
    fn import_delegates_to_the_provider() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        let imported = access
            .import_package(
                owner,
                vec![PackageUploadFile {
                    path: "package.json".into(),
                    bytes: b"{}".to_vec(),
                }],
            )
            .unwrap();
        assert_eq!(imported.installation_id, installation);
        assert_eq!(imported.id, "com.rumahl.test");
    }

    #[test]
    fn invoke_capability_delegates_to_the_provider() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.test.run".into(), None)
                .unwrap(),
            CapabilityResult {
                outcome: CapabilityOutcome::Invoked,
                result: Some("{\"ran\":true}".into()),
                browser: None
            }
        );
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.unknown".into(), None)
                .unwrap(),
            CapabilityResult {
                outcome: CapabilityOutcome::Denied,
                result: None,
                browser: None
            }
        );
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.test.browser".into(), None)
                .unwrap()
                .browser,
            Some(BrowserDelivery {
                app_id: "com.rumahl.test".into(),
                installation_id: installation.to_string()
            })
        );
    }

    #[test]
    fn runtime_control_reports_state_and_denies_other_installations() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        assert_eq!(
            access
                .runtime(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    RuntimeAction::Status
                )
                .unwrap(),
            RuntimeState::Running
        );
        assert_eq!(
            access
                .runtime(
                    owner,
                    "com.rumahl.test",
                    &InstallationId::new().to_string(),
                    RuntimeAction::Status
                )
                .unwrap_err(),
            AppAccessError::Denied
        );
    }

    #[test]
    fn uninstall_requires_identity_and_matching_installation() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        access
            .uninstall(owner, "com.rumahl.test", &installation.to_string())
            .unwrap();
        assert_eq!(
            access
                .uninstall(owner, "com.rumahl.test", &InstallationId::new().to_string())
                .unwrap_err(),
            AppAccessError::Denied
        );
        assert_eq!(
            access
                .uninstall(
                    ShellIdentity {
                        session_id: SessionId::new(),
                        ..owner
                    },
                    "com.rumahl.test",
                    &installation.to_string(),
                )
                .unwrap_err(),
            AppAccessError::Denied
        );
    }

    #[test]
    fn leases_bind_host_installation_session_and_expire() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        let launch = access
            .launch(owner, "com.rumahl.test", &installation.to_string(), None)
            .unwrap();
        let url = Url::parse(&launch.frame_url).unwrap();
        let host = access.authority(installation);
        assert!(access.accepts_host(&host));
        assert!(!access.accepts_host("localhost:8443"));
        assert!(!access.accepts_host(&format!("{host}.evil.test")));
        assert!(access.resource(&host, url.path()).is_ok());
        assert!(access.resource("localhost:8443", url.path()).is_err());
        assert!(
            access
                .resource(&access.authority(InstallationId::new()), url.path())
                .is_err()
        );
        assert!(
            access
                .resource(&host, &format!("/launch/{}/../secret.json", launch.lease))
                .is_err()
        );
        let other = ShellIdentity {
            session_id: SessionId::new(),
            ..owner
        };
        assert!(
            access
                .launch(
                    other,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease)
                )
                .is_err()
        );
        assert!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &InstallationId::new().to_string(),
                    Some(&launch.lease)
                )
                .is_err()
        );
        assert_eq!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease)
                )
                .unwrap()
                .frame_url,
            launch.frame_url
        );
        let response = access.asset_response(&host, access.resource(&host, url.path()).unwrap());
        assert_eq!(response.headers()["access-control-allow-origin"], "null");
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .starts_with("sandbox allow-scripts;")
        );
        access
            .leases
            .lock()
            .unwrap()
            .get_mut(&launch.lease)
            .unwrap()
            .expires = Instant::now() - Duration::from_secs(1);
        assert!(access.resource(&host, url.path()).is_err());
        assert!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease)
                )
                .is_err()
        );
    }

    #[test]
    fn settings_are_scoped_to_the_installed_app() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();

        let settings = access
            .settings(owner, "com.rumahl.test", &installation.to_string())
            .unwrap();
        assert_eq!(settings.manifest.len(), 1);
        assert_eq!(settings.manifest[0].key, "server.url");
        assert_eq!(settings.manifest[0].kind, "text");
        assert!(settings.manifest[0].required);

        // Unknown installation or unauthenticated identity is denied.
        assert!(
            access
                .settings(owner, "com.rumahl.test", &InstallationId::new().to_string())
                .is_err()
        );
        assert!(
            access
                .settings(
                    ShellIdentity {
                        session_id: SessionId::new(),
                        ..owner
                    },
                    "com.rumahl.test",
                    &installation.to_string()
                )
                .is_err()
        );
    }
    #[test]
    fn surface_leases_are_bound_to_the_surface_and_app() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();

        let launch = access
            .launch(owner, "com.rumahl.test", &installation.to_string(), None)
            .unwrap();
        let surface = access
            .surface(
                owner,
                &installation.to_string(),
                "com.rumahl.test.desktop",
                None,
            )
            .unwrap();
        assert_eq!(surface.surface, "com.rumahl.test.desktop");
        assert_eq!(surface.lease.len(), 64);
        assert!(
            surface
                .frame_url
                .ends_with(&format!("/launch/{}/web/surface.html", surface.lease)[..])
        );

        let renewed = access
            .surface(
                owner,
                &installation.to_string(),
                "com.rumahl.test.desktop",
                Some(&surface.lease),
            )
            .unwrap();
        assert_eq!(renewed.lease, surface.lease);

        assert!(
            access
                .surface(
                    owner,
                    &installation.to_string(),
                    "com.rumahl.test.desktop",
                    Some(&launch.lease),
                )
                .is_err()
        );
        assert!(
            access
                .surface(
                    owner,
                    &installation.to_string(),
                    "com.rumahl.test.missing",
                    None
                )
                .is_err()
        );
    }

    #[test]
    fn surface_token_resolves_only_on_its_app_origin() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            "apps.localhost",
        )
        .unwrap();
        let surface = access
            .surface(
                owner,
                &installation.to_string(),
                "com.rumahl.test.desktop",
                None,
            )
            .unwrap();
        let authority = format!("{installation}.apps.localhost:8443");
        let asset = access
            .resource(
                &authority,
                &format!("/launch/{}/web/surface.html", surface.lease),
            )
            .unwrap();
        assert_eq!(asset.content_type, "text/html; charset=utf-8");
        assert!(
            access
                .resource(
                    "other.apps.localhost:8443",
                    &format!("/launch/{}/web/surface.html", surface.lease)
                )
                .is_err()
        );
    }
}
