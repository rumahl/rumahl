//! Composition root for the first locally hosted OS shell.
use rumahl_account_auth::{
    LocalAccountAdministrationService, PasswordAuthenticationError, PasswordAuthenticationService,
    PasswordBlocklist, SessionToken, StoredSessionCredentialResolver,
};
use rumahl_core::{
    AccountStateRepository, BrowserProfileId, PlatformSnapshotRepository,
    ShellPreferencesRepository, UnixTimestamp, UserId, WorkspaceRepository,
};
use rumahl_persistence_sqlite::{
    BrowserSessionError, SqliteAccountStateRepository, SqliteBrowserSessionRepository,
    SqliteLocalAccountAdministrationRepository, SqlitePasswordCredentialRepository,
    SqliteSessionCredentialRepository, SqliteShellPreferences, SqliteSnapshotRepository,
};
use rumahl_platform_api::{LocalSessionAuthenticator, ShellSnapshotProvider, ShellSnapshotSubject};
use rumahl_platform_web::{
    BrowserSessions, PlatformShellBackend, SESSION_SECONDS, ShellBackend, ShellBackendError,
    WidgetFrameResolver,
};
use rumahl_ui_contracts::{
    ResolvedTheme, ShellApp, ShellMode, ShellSnapshot, ShellSystemStatus, ShellTheme, ShellUser,
    SystemProtectionStatus, theme_by_id,
};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path, sync::Arc};
use zeroize::Zeroizing;

pub type ServiceError = Box<dyn std::error::Error + Send + Sync>;

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

pub struct LocalBrowserSessions {
    accounts: SqliteAccountStateRepository,
    passwords:
        PasswordAuthenticationService<SqlitePasswordCredentialRepository, LocalPasswordBlocklist>,
    sessions: SqliteBrowserSessionRepository,
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
        })
    }
}
impl BrowserSessions for LocalBrowserSessions {
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
        let proof = self
            .passwords
            .authenticate(&state, username, std::mem::take(&mut *password), now)
            .map_err(|error| match error {
                PasswordAuthenticationError::InvalidCredentials => ShellBackendError::Unauthorized,
                _ => ShellBackendError::Unavailable,
            })?;
        let expires = UnixTimestamp::from_seconds(
            now.as_seconds()
                .checked_add(SESSION_SECONDS)
                .ok_or(ShellBackendError::Unavailable)?,
        );
        self.sessions
            .create(proof, expires)
            .map(|token| token.encode())
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
        self.passwords
            .authenticate(&state, &username, std::mem::take(&mut *password), now)
            .map_err(|error| match error {
                PasswordAuthenticationError::InvalidCredentials => ShellBackendError::Unauthorized,
                _ => ShellBackendError::Unavailable,
            })?;
        Ok(())
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
            .map_err(|_| ShellBackendError::Unavailable)
    }
}

pub struct PersistentShellSnapshots {
    accounts: SqliteAccountStateRepository,
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
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(accounts)?,
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
            &self.build_id,
            &catalog_revision,
            theme.id(),
            mode.as_str(),
            workspace.as_deref().unwrap_or(""),
        ))?);
        Ok(ShellSnapshot::new(
            &self.build_id,
            revision,
            ShellUser::new(user.display_name(), &self.locale)?,
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
            vec![],
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
    ["com.rumahl.default", "com.rumahl.classic"]
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

pub mod apps;
pub mod install;
