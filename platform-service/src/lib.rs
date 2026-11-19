//! Composition root for the first locally hosted OS shell.
use rumahl_account_auth::{
    LocalAccountAdministrationService, PasswordAuthenticationError, PasswordAuthenticationService,
    PasswordBlocklist, SessionToken, StoredSessionCredentialResolver,
};
use rumahl_core::{
    AccountStateRepository, AuditAction, AuditActor, AuditLog, AuditOutcome, BrowserProfileId,
    InstalledAppSnapshot, NewAuditEvent, OsMode, OsModeRepository, OsModeSettings,
    OsModeStoreError, PlatformSnapshotRepository, PreferenceScope, ShellPreferencesRepository,
    UnixTimestamp, UserId, UserRole, WorkspaceRepository,
};
use rumahl_persistence_sqlite::{
    BrowserSessionError, SqliteAccountRoles, SqliteAccountStateRepository,
    SqliteBrowserSessionRepository, SqliteLocalAccountAdministrationRepository,
    SqlitePasswordCredentialRepository, SqliteSessionCredentialRepository, SqliteShellPreferences,
    SqliteSnapshotRepository,
};
use rumahl_platform_api::{LocalSessionAuthenticator, ShellSnapshotProvider, ShellSnapshotSubject};
use rumahl_platform_web::{
    BrowserSessions, PlatformShellBackend, SESSION_SECONDS, ShellBackend, ShellBackendError,
    WidgetFrameResolver,
};
use rumahl_ui_contracts::{
    ExtensionContribution, ExtensionSize, ResolvedTheme, ShellApp, ShellMode, ShellSnapshot,
    ShellSystemStatus, ShellTheme, ShellUser, ShellUserRole, SurfaceIcon, SurfaceKind,
    SurfacePlacement, SurfaceSlot, SurfaceTone, SystemProtectionStatus, theme_by_id,
};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashSet, path::Path, sync::Arc};
use zeroize::Zeroizing;

pub type ServiceError = Box<dyn std::error::Error + Send + Sync>;

mod administration;
pub use administration::LocalAdministration;

/// A locally supplied blocklist is required when provisioning accounts. Login
/// uses existing verifiers and does not depend on a cloud password service.
#[derive(Clone, Default)]
pub struct LocalPasswordBlocklist(pub HashSet<String>);
impl PasswordBlocklist for LocalPasswordBlocklist {
    fn contains(&self, value: &str) -> bool {
        self.0.contains(value)
    }
}

pub fn provision(
    path: &Path,
    username: &str,
    display_name: &str,
    password: String,
    blocklist: LocalPasswordBlocklist,
) -> Result<(), ServiceError> {
    let accounts = SqliteAccountStateRepository::open(path)?;
    let mut state = accounts.load()?;
    let admin = LocalAccountAdministrationService::new(
        SqliteLocalAccountAdministrationRepository::open(path)?,
        blocklist,
    );
    admin.provision_password_account(
        &mut state,
        username,
        display_name,
        password,
        UnixTimestamp::now()?,
    )?;
    Ok(())
}

/// State-volume databases, in backup order.
pub const STATE_DATABASES: [&str; 6] = [
    "accounts.sqlite",
    "platform.sqlite",
    "preferences.sqlite",
    "files.sqlite",
    "files-index.sqlite",
    "audit.sqlite",
];

/// Writes a consistent snapshot of every state database into `destination`.
///
/// The destination is forced owner-only (`0700`) because backups contain
/// accounts and personal files. Every source is schema-checked first, so a
/// database written by a newer binary is refused instead of copied. Snapshots
/// use the SQLite online backup API, never a live file copy.
pub fn backup_state(state_dir: &Path, destination: &Path) -> Result<(), ServiceError> {
    backup_state_with_home(state_dir, destination, &user_homes::home_root())
}

/// [`backup_state`] with an explicit home root, for tests and callers that
/// manage the layout themselves.
pub fn backup_state_with_home(
    state_dir: &Path,
    destination: &Path,
    home_root: &Path,
) -> Result<(), ServiceError> {
    std::fs::create_dir_all(destination)?;
    let metadata = std::fs::symlink_metadata(destination)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("backup destination must be a directory").into());
    }
    std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))?;
    for name in STATE_DATABASES {
        let source = state_dir.join(name);
        if !source.exists() {
            continue;
        }
        rumahl_persistence_sqlite::verify_schema(&source).map_err(|error| {
            std::io::Error::other(format!("database schema check failed: {error}"))
        })?;
        rumahl_persistence_sqlite::backup_database(&source, &destination.join(name))?;
    }
    backup_account_homes(&state_dir.join("accounts.sqlite"), home_root, destination)
}

/// Copies each account's home into `destination/home`.
///
/// The home is the source of truth for user documents, so a state backup that
/// skipped it would silently lose them. Only accounts known to this deployment
/// are copied (never every directory that happens to live under `/home`), and
/// symlinks are never followed: a link in a home must not pull in files from
/// outside it.
fn backup_account_homes(
    accounts: &Path,
    home_root: &Path,
    destination: &Path,
) -> Result<(), ServiceError> {
    if !home_root.is_dir() {
        return Ok(());
    }
    let home_backup = destination.join("home");
    if let (Ok(root), Ok(target)) = (home_root.canonicalize(), destination.canonicalize())
        && target.starts_with(&root)
    {
        return Err(
            std::io::Error::other("backup destination must not be inside the home root").into(),
        );
    }
    let state = SqliteAccountStateRepository::open(accounts)?.load()?;
    std::fs::create_dir_all(&home_backup)?;
    std::fs::set_permissions(&home_backup, std::fs::Permissions::from_mode(0o700))?;
    for account in state.accounts().accounts() {
        let source = home_root.join(account.username().as_str());
        if !source.is_dir() {
            continue;
        }
        copy_home_tree(&source, &home_backup.join(account.username().as_str()), 0)?;
    }
    Ok(())
}

/// Bounded, symlink-safe copy. Backups are owner-only and keep the modes the
/// files would have in a fresh home.
fn copy_home_tree(source: &Path, destination: &Path, depth: usize) -> Result<(), ServiceError> {
    const MAX_DEPTH: usize = 128;
    if depth > MAX_DEPTH {
        return Err(std::io::Error::other("home backup directory nesting is too deep").into());
    }
    std::fs::create_dir_all(destination)?;
    std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            copy_home_tree(&entry.path(), &target, depth + 1)?;
        } else if metadata.is_file() {
            std::fs::copy(entry.path(), &target)?;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

/// Resolves the apps data volume root.
///
/// `RUMAHL_APPS_ROOT` overrides it (development and tests); otherwise the
/// product path `/apps` is used when it exists. `None` disables app-data
/// browsing rather than silently writing somewhere else.
pub fn apps_data_root() -> Option<std::path::PathBuf> {
    match std::env::var_os("RUMAHL_APPS_ROOT") {
        Some(value) => Some(std::path::PathBuf::from(value)),
        None => {
            let default = std::path::PathBuf::from("/apps");
            default.is_dir().then_some(default)
        }
    }
}

pub struct LocalBrowserSessions {
    accounts: SqliteAccountStateRepository,
    passwords:
        PasswordAuthenticationService<SqlitePasswordCredentialRepository, LocalPasswordBlocklist>,
    sessions: SqliteBrowserSessionRepository,
    /// Optional append-only audit trail; never receives secrets.
    audit: Option<Arc<dyn AuditLog>>,
}
impl LocalBrowserSessions {
    pub fn open(path: &Path) -> Result<Self, ServiceError> {
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(path)?,
            passwords: PasswordAuthenticationService::new(
                SqlitePasswordCredentialRepository::open(path)?,
                LocalPasswordBlocklist::default(),
            )?,
            sessions: SqliteBrowserSessionRepository::open(path)?,
            audit: None,
        })
    }

    /// Records authentication events to the audit trail.
    pub fn with_audit(mut self, audit: Arc<dyn AuditLog>) -> Self {
        self.audit = Some(audit);
        self
    }

    fn record(
        &self,
        actor: AuditActor,
        action: AuditAction,
        outcome: AuditOutcome,
        target: Option<&str>,
    ) {
        let Some(audit) = &self.audit else {
            return;
        };
        let Ok(at) = UnixTimestamp::now() else {
            return;
        };
        let mut event = NewAuditEvent::new(at, actor, action, outcome);
        if let Some(target) = target
            && let Ok(with_target) = event.clone().with_target(target)
        {
            event = with_target;
        }
        let _ = audit.record(event);
    }
}
impl BrowserSessions for LocalBrowserSessions {
    fn login_accounts(&self) -> Result<Vec<rumahl_platform_web::LoginAccount>, ShellBackendError> {
        let state = self
            .accounts
            .load()
            .map_err(|_| ShellBackendError::Unavailable)?;
        Ok(state
            .accounts()
            .accounts()
            .iter()
            .filter(|account| account.can_authenticate())
            .map(|account| rumahl_platform_web::LoginAccount {
                username: account.username().as_str().to_owned(),
                display_name: account.display_name().to_owned(),
            })
            .collect())
    }

    fn login(
        &self,
        username: &str,
        password: String,
    ) -> Result<Zeroizing<String>, ShellBackendError> {
        let mut password = Zeroizing::new(password);
        let now = UnixTimestamp::now().map_err(|_| ShellBackendError::Unavailable)?;
        let state = self
            .accounts
            .load()
            .map_err(|_| ShellBackendError::Unavailable)?;
        let proof =
            match self
                .passwords
                .authenticate(&state, username, std::mem::take(&mut *password), now)
            {
                Ok(proof) => proof,
                Err(error) => {
                    if matches!(error, PasswordAuthenticationError::InvalidCredentials) {
                        self.record(
                            AuditActor::System,
                            AuditAction::SignInFailed,
                            AuditOutcome::Denied,
                            None,
                        );
                    }
                    return Err(match error {
                        PasswordAuthenticationError::InvalidCredentials => {
                            ShellBackendError::Unauthorized
                        }
                        _ => ShellBackendError::Unavailable,
                    });
                }
            };
        let user_id = *proof.user_id();
        let expires = UnixTimestamp::from_seconds(
            now.as_seconds()
                .checked_add(SESSION_SECONDS)
                .ok_or(ShellBackendError::Unavailable)?,
        );
        self.sessions
            .create(proof, expires)
            .map(|token| {
                self.record(
                    AuditActor::User(user_id),
                    AuditAction::SignIn,
                    AuditOutcome::Success,
                    None,
                );
                token.encode()
            })
            .map_err(|error| match error {
                BrowserSessionError::InvalidProof => ShellBackendError::Unauthorized,
                BrowserSessionError::Storage => ShellBackendError::Unavailable,
            })
    }
    fn reauthenticate(&self, user_id: UserId, password: String) -> Result<(), ShellBackendError> {
        let mut password = Zeroizing::new(password);
        let now = UnixTimestamp::now().map_err(|_| ShellBackendError::Unavailable)?;
        let state = self
            .accounts
            .load()
            .map_err(|_| ShellBackendError::Unavailable)?;
        let username = state
            .accounts()
            .accounts()
            .iter()
            .find(|account| *account.user_id() == user_id)
            .map(|account| account.username().as_str().to_owned())
            .ok_or(ShellBackendError::Unauthorized)?;
        match self
            .passwords
            .authenticate(&state, &username, std::mem::take(&mut *password), now)
        {
            Ok(_) => {
                self.record(
                    AuditActor::User(user_id),
                    AuditAction::Reauthenticate,
                    AuditOutcome::Success,
                    None,
                );
                Ok(())
            }
            Err(error) => {
                if matches!(error, PasswordAuthenticationError::InvalidCredentials) {
                    self.record(
                        AuditActor::User(user_id),
                        AuditAction::ReauthenticateFailed,
                        AuditOutcome::Denied,
                        None,
                    );
                    Err(ShellBackendError::Unauthorized)
                } else {
                    Err(ShellBackendError::Unavailable)
                }
            }
        }
    }
    fn logout(&self, credential: &str) -> Result<(), ShellBackendError> {
        let Ok(token) = SessionToken::parse(credential) else {
            return Ok(());
        };
        self.sessions
            .revoke(
                &token,
                UnixTimestamp::now().map_err(|_| ShellBackendError::Unavailable)?,
            )
            .map_err(|_| ShellBackendError::Unavailable)?;
        self.record(
            AuditActor::System,
            AuditAction::SignOut,
            AuditOutcome::Success,
            None,
        );
        Ok(())
    }
}

/// Wraps a mode repository so every accepted change is written to the audit
/// trail. A failed change is recorded as a `failure` without rolling back.
pub struct AuditedOsModeRepository {
    inner: Arc<dyn OsModeRepository>,
    audit: Arc<dyn AuditLog>,
}

impl AuditedOsModeRepository {
    pub fn new(inner: Arc<dyn OsModeRepository>, audit: Arc<dyn AuditLog>) -> Self {
        Self { inner, audit }
    }

    fn record(&self, user: UserId, mode: Option<OsMode>, outcome: AuditOutcome) {
        let Ok(at) = UnixTimestamp::now() else {
            return;
        };
        let mut event = NewAuditEvent::new(
            at,
            AuditActor::User(user),
            AuditAction::OsModeChanged,
            outcome,
        );
        if let Some(target) = mode.map(|mode| mode.as_str())
            && let Ok(with_target) = event.clone().with_target(target)
        {
            event = with_target;
        }
        let _ = self.audit.record(event);
    }
}

impl OsModeRepository for AuditedOsModeRepository {
    fn load(
        &self,
        user: UserId,
        device: BrowserProfileId,
    ) -> Result<OsModeSettings, OsModeStoreError> {
        self.inner.load(user, device)
    }

    fn save(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        mode: Option<OsMode>,
    ) -> Result<OsModeSettings, OsModeStoreError> {
        let result = self.inner.save(user, device, revision, scope, mode);
        let effective = result
            .as_ref()
            .ok()
            .map(OsModeSettings::effective_mode)
            .or(mode);
        let outcome = if result.is_ok() {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failure
        };
        self.record(user, effective, outcome);
        result
    }
}

fn surface_contributions(
    visible: &[InstalledAppSnapshot],
) -> Result<Vec<ExtensionContribution>, ServiceError> {
    let mut contributions = Vec::new();
    let mut ids = HashSet::new();
    for app in visible {
        for surface in app.manifest().surfaces() {
            if contributions.len() >= 128 || !ids.insert(surface.id().as_str().to_owned()) {
                continue;
            }
            contributions.push(
                ExtensionContribution::surface(
                    surface.id().as_str(),
                    app.manifest().display_name(),
                    app.identity().app_id().as_str(),
                    surface_slot(surface.slot()),
                    surface_kind(surface.kind()),
                    surface_size(surface.size()),
                    surface_placement(surface.placement()),
                    surface_tone(surface.tone()),
                    surface_icon(surface.icon()),
                    surface
                        .entrypoint()
                        .map(|entrypoint| entrypoint.as_str().to_owned()),
                )
                .map_err(|_| std::io::Error::other("invalid app surface"))?,
            );
        }
    }
    Ok(contributions)
}

fn surface_slot(value: rumahl_core::SurfaceSlot) -> SurfaceSlot {
    match value {
        rumahl_core::SurfaceSlot::Desktop => SurfaceSlot::Desktop,
        rumahl_core::SurfaceSlot::Launcher => SurfaceSlot::Launcher,
        rumahl_core::SurfaceSlot::Both => SurfaceSlot::Both,
    }
}

fn surface_kind(value: rumahl_core::SurfaceKind) -> SurfaceKind {
    match value {
        rumahl_core::SurfaceKind::Companion => SurfaceKind::Companion,
        rumahl_core::SurfaceKind::Badge => SurfaceKind::Badge,
        rumahl_core::SurfaceKind::Panel => SurfaceKind::Panel,
    }
}

fn surface_size(value: rumahl_core::SurfaceSize) -> ExtensionSize {
    match value {
        rumahl_core::SurfaceSize::Small => ExtensionSize::Small,
        rumahl_core::SurfaceSize::Medium => ExtensionSize::Medium,
        rumahl_core::SurfaceSize::Large => ExtensionSize::Large,
    }
}

fn surface_placement(value: rumahl_core::SurfacePlacement) -> SurfacePlacement {
    match value {
        rumahl_core::SurfacePlacement::TopLeft => SurfacePlacement::TopLeft,
        rumahl_core::SurfacePlacement::TopRight => SurfacePlacement::TopRight,
        rumahl_core::SurfacePlacement::BottomLeft => SurfacePlacement::BottomLeft,
        rumahl_core::SurfacePlacement::BottomRight => SurfacePlacement::BottomRight,
    }
}

fn surface_tone(value: rumahl_core::SurfaceTone) -> SurfaceTone {
    match value {
        rumahl_core::SurfaceTone::Accent => SurfaceTone::Accent,
        rumahl_core::SurfaceTone::Neutral => SurfaceTone::Neutral,
        rumahl_core::SurfaceTone::Warm => SurfaceTone::Warm,
        rumahl_core::SurfaceTone::Cool => SurfaceTone::Cool,
        rumahl_core::SurfaceTone::Rose => SurfaceTone::Rose,
        rumahl_core::SurfaceTone::Mint => SurfaceTone::Mint,
    }
}

fn surface_icon(value: rumahl_core::SurfaceIcon) -> SurfaceIcon {
    match value {
        rumahl_core::SurfaceIcon::Dot => SurfaceIcon::Dot,
        rumahl_core::SurfaceIcon::Star => SurfaceIcon::Star,
        rumahl_core::SurfaceIcon::Bolt => SurfaceIcon::Bolt,
        rumahl_core::SurfaceIcon::Leaf => SurfaceIcon::Leaf,
        rumahl_core::SurfaceIcon::Moon => SurfaceIcon::Moon,
        rumahl_core::SurfaceIcon::Heart => SurfaceIcon::Heart,
        rumahl_core::SurfaceIcon::Orbit => SurfaceIcon::Orbit,
        rumahl_core::SurfaceIcon::Pulse => SurfaceIcon::Pulse,
    }
}

pub(crate) fn shell_role(role: UserRole) -> ShellUserRole {
    match role {
        UserRole::PreAuthentication | UserRole::User => ShellUserRole::User,
        UserRole::Manager => ShellUserRole::Manager,
        UserRole::Administrator => ShellUserRole::Administrator,
        UserRole::Owner => ShellUserRole::Owner,
    }
}

pub struct PersistentShellSnapshots {
    accounts: SqliteAccountStateRepository,
    roles: SqliteAccountRoles,
    platform: SqliteSnapshotRepository,
    build_id: String,
    locale: String,
    preferences: Option<Arc<dyn ShellPreferencesRepository>>,
    workspace: Option<Arc<dyn WorkspaceRepository>>,
}
impl PersistentShellSnapshots {
    pub fn open(
        accounts: &Path,
        platform: &Path,
        build_id: &str,
        locale: &str,
    ) -> Result<Self, ServiceError> {
        // Validate configuration before opening the listener.
        ShellUser::new("configuration", locale)?;
        ShellSnapshot::new(
            build_id,
            "startup",
            ShellUser::new("configuration", locale)?,
            stock_theme()?,
            vec![],
            None,
            ShellSystemStatus::new(SystemProtectionStatus::Attention, 0, 1, None)?,
            vec![],
        )?;
        let roles = SqliteAccountRoles::open(accounts)?;
        roles.ensure_owner()?;
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(accounts)?,
            roles,
            platform: SqliteSnapshotRepository::open(platform)?,
            build_id: build_id.into(),
            locale: locale.into(),
            preferences: None,
            workspace: None,
        })
    }

    /// Attaches the presentation-preference store so the snapshot can carry the
    /// account-scoped theme. Device overrides remain a client-side concern.
    pub fn with_preferences(mut self, preferences: Arc<dyn ShellPreferencesRepository>) -> Self {
        self.preferences = Some(preferences);
        self
    }

    /// Attaches the workspace store so the device-scoped window layout can be
    /// rendered on the first paint instead of after a client fetch.
    pub fn with_workspace(mut self, workspace: Arc<dyn WorkspaceRepository>) -> Self {
        self.workspace = Some(workspace);
        self
    }

    fn resolve_workspace(
        &self,
        user_id: &UserId,
        device: Option<BrowserProfileId>,
    ) -> Option<String> {
        const SENTINEL: &str = "00000000-0000-4000-8000-000000000000";
        let workspace = self.workspace.as_ref()?;
        let profile = device.or_else(|| BrowserProfileId::parse(SENTINEL))?;
        let preferences = workspace.load_workspace(*user_id, profile).ok()?;
        preferences.device.or(preferences.user)
    }

    fn resolve_preferences(
        &self,
        user_id: &UserId,
        device: Option<BrowserProfileId>,
    ) -> Option<rumahl_core::ShellPreferences> {
        // Without a device cookie, probe a sentinel profile for the account value.
        const SENTINEL: &str = "00000000-0000-4000-8000-000000000000";
        let preferences = self.preferences.as_ref()?;
        let profile = device.or_else(|| BrowserProfileId::parse(SENTINEL))?;
        preferences.load(*user_id, profile).ok()
    }

    fn resolve_theme(&self, user_id: &UserId, device: Option<BrowserProfileId>) -> ResolvedTheme {
        self.resolve_preferences(user_id, device)
            .and_then(|value| theme_by_id(value.effective_theme()))
            .unwrap_or_else(ResolvedTheme::stock)
    }

    fn resolve_mode(&self, user_id: &UserId, device: Option<BrowserProfileId>) -> ShellMode {
        match self
            .resolve_preferences(user_id, device)
            .map(|value| value.effective_mode())
        {
            Some(rumahl_core::ShellMode::Launcher) => ShellMode::Launcher,
            _ => ShellMode::Desktop,
        }
    }
    pub fn for_user(
        &self,
        user_id: &UserId,
        device: Option<BrowserProfileId>,
    ) -> Result<ShellSnapshot, ServiceError> {
        let state = self.accounts.load()?;
        let user = state
            .accounts()
            .get(user_id)
            .filter(|a| a.can_authenticate())
            .ok_or_else(|| std::io::Error::other("account unavailable"))?;
        let role = self.roles.role(*user_id)?;
        let platform = self.platform.load()?;
        let now_seconds = UnixTimestamp::now()?;
        let identity = state
            .sessions()
            .sessions_for_user(user_id)
            .find(|session| session.is_active_at(now_seconds))
            .map(|session| rumahl_platform_web::ShellIdentity {
                user_id: *user_id,
                session_id: *session.session_id(),
            });
        let visible = match (platform.as_ref(), identity) {
            (Some(platform), Some(identity)) => apps::authorized_apps(platform, identity)
                .map_err(|_| std::io::Error::other("app catalog unavailable"))?,
            _ => vec![],
        };
        let catalog_revision: Vec<_> = visible
            .iter()
            .map(|app| {
                (
                    app.identity().app_id().to_string(),
                    app.installation_id().to_string(),
                    app.manifest().display_name(),
                    app.manifest().version().to_string(),
                )
            })
            .collect();
        let shell_apps = visible
            .iter()
            .map(|app| {
                ShellApp::new(
                    app.identity().app_id().to_string(),
                    app.manifest().display_name(),
                    true,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let contributions = surface_contributions(&visible)?;
        let surface_revision = serde_json::to_vec(&contributions)
            .map_err(|_| std::io::Error::other("invalid app surfaces"))?;
        let count = visible.len();
        let now = UnixTimestamp::now()?
            .as_seconds()
            .checked_mul(1000)
            .ok_or_else(|| std::io::Error::other("invalid clock"))?;
        let theme = self.resolve_theme(user_id, device);
        let mode = self.resolve_mode(user_id, device);
        let workspace = self.resolve_workspace(user_id, device);
        let revision = sha256_hex(&serde_json::to_vec(&(
            user_id.to_string(),
            user.display_name(),
            &self.locale,
            role.as_str(),
            &self.build_id,
            &catalog_revision,
            &surface_revision,
            theme.id(),
            mode.as_str(),
            workspace.as_deref().unwrap_or(""),
        ))?);
        Ok(ShellSnapshot::new(
            &self.build_id,
            revision,
            ShellUser::new(user.display_name(), &self.locale)?.with_role(shell_role(role)),
            ShellTheme::from_theme(theme_stylesheet(theme.id()).0, &theme)?,
            shell_apps,
            workspace,
            // No system health assessment exists yet: never claim the device is secure.
            ShellSystemStatus::new(
                SystemProtectionStatus::Attention,
                u32::try_from(count)?,
                now,
                None,
            )?,
            contributions,
        )?
        .with_mode(mode))
    }
}
impl ShellSnapshotProvider for PersistentShellSnapshots {
    type Error = std::io::Error;
    fn load_for_user(&self, subject: &ShellSnapshotSubject) -> Result<ShellSnapshot, Self::Error> {
        self.for_user(subject.user_id(), subject.device())
            .map_err(|_| std::io::Error::other("shell snapshot unavailable"))
    }
}

pub fn shell_backend(
    accounts: &Path,
    platform: &Path,
    build_id: &str,
    locale: &str,
) -> Result<Arc<dyn ShellBackend>, ServiceError> {
    let auth = LocalSessionAuthenticator::new(
        StoredSessionCredentialResolver::new(SqliteSessionCredentialRepository::open(accounts)?),
        SqliteAccountStateRepository::open(accounts)?,
    );
    let store = Arc::new(SqliteShellPreferences::open(
        accounts
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("preferences.sqlite"),
    )?);
    Ok(Arc::new(PlatformShellBackend::new(
        auth,
        PersistentShellSnapshots::open(accounts, platform, build_id, locale)?
            .with_preferences(store.clone())
            .with_workspace(store.clone()),
    )))
}

pub fn stock_stylesheet() -> (String, String) {
    theme_stylesheet("com.rumahl.default")
}

/// Compiles a built-in theme into a content-addressed stylesheet.
pub fn theme_stylesheet(id: &str) -> (String, String) {
    let theme = theme_by_id(id).unwrap_or_else(ResolvedTheme::stock);
    let css = theme.compile_css();
    let path = format!("/shell/themes/sha256-{}.css", sha256_hex(css.as_bytes()));
    (path, css)
}

/// All built-in stylesheets that must be served for the account theme to render
/// correctly on the first paint (no flash of the default theme).
pub fn theme_stylesheets() -> Vec<(String, String)> {
    ["com.rumahl.default"]
        .into_iter()
        .map(theme_stylesheet)
        .collect()
}

fn stock_theme() -> Result<ShellTheme, rumahl_ui_contracts::ShellSnapshotError> {
    ShellTheme::from_theme(stock_stylesheet().0, &ResolvedTheme::stock())
}

pub struct NoWidgets;
impl WidgetFrameResolver for NoWidgets {
    fn resolve(&self, _: UserId, _: &str, _: &str) -> Option<String> {
        None
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub mod app_grants;
mod app_runtime;
pub mod apps;
pub mod channel;
pub mod file_migration;
pub mod host_files;
pub mod install;
pub mod policy_engine;
pub mod user_homes;

#[cfg(test)]
mod surface_tests {
    use super::*;
    use rumahl_core::{AppId, InstalledApp, PackagePath, PublisherId};
    use rumahl_core::{
        AppManifest, AppManifestValidator, AppVersion, RuntimeDescriptor, RuntimeEntrypoint,
        RuntimeEntrypointId, SurfaceDeclaration, SurfaceIcon, SurfaceKind, SurfacePlacement,
        SurfaceSize, SurfaceSlot, SurfaceTone,
    };

    fn installed_app_with_surface() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("index.html").unwrap(),
            ))
            .unwrap();
        let mut manifest = AppManifest::new(
            AppId::parse("com.example.clock").unwrap(),
            PublisherId::parse("com.example").unwrap(),
            AppVersion::new(1, 0, 0),
            "Clock",
            runtime,
        )
        .unwrap();
        manifest
            .add_surface(
                SurfaceDeclaration::new(
                    rumahl_core::ContributionId::parse("com.example.clock.desktop").unwrap(),
                    SurfaceSlot::Both,
                    SurfaceKind::Companion,
                    SurfaceSize::Medium,
                    SurfacePlacement::BottomRight,
                    SurfaceTone::Accent,
                    SurfaceIcon::Pulse,
                )
                .with_entrypoint(Some(RuntimeEntrypointId::parse("main").unwrap())),
            )
            .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn projects_installed_app_surfaces_into_shell_contributions() {
        let app = InstalledAppSnapshot::capture(&installed_app_with_surface());
        let contributions = surface_contributions(&[app]).unwrap();
        assert_eq!(contributions.len(), 1);
        let contribution = &contributions[0];
        assert_eq!(contribution.id(), "com.example.clock.desktop");
        let json = serde_json::to_value(contribution).unwrap();
        assert_eq!(json["app"], "com.example.clock");
        assert_eq!(json["entrypoint"], "main");
    }
}
