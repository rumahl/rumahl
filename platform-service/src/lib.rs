//! Composition root for the first locally hosted OS shell.
use rumahl_account_auth::{
    LocalAccountAdministrationService, PasswordAuthenticationError, PasswordAuthenticationService,
    PasswordBlocklist, SessionToken, StoredSessionCredentialResolver,
};
use rumahl_core::{AccountStateRepository, PlatformSnapshotRepository, UnixTimestamp, UserId};
use rumahl_persistence_sqlite::{
    BrowserSessionError, SqliteAccountStateRepository, SqliteBrowserSessionRepository,
    SqliteLocalAccountAdministrationRepository, SqlitePasswordCredentialRepository,
    SqliteSessionCredentialRepository, SqliteSnapshotRepository,
};
use rumahl_platform_api::{LocalSessionAuthenticator, ShellSnapshotProvider, ShellSnapshotSubject};
use rumahl_platform_web::{
    BrowserSessions, PlatformShellBackend, SESSION_SECONDS, ShellBackend, ShellBackendError,
    WidgetFrameResolver,
};
use rumahl_ui_contracts::{
    ResolvedTheme, ShellSnapshot, ShellSystemStatus, ShellTheme, ShellUser, SystemProtectionStatus,
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
            ShellSystemStatus::new(SystemProtectionStatus::Attention, 0, 1, None)?,
            vec![],
        )?;
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(accounts)?,
            platform: SqliteSnapshotRepository::open(platform)?,
            build_id: build_id.into(),
            locale: locale.into(),
        })
    }
    pub fn for_user(&self, user_id: &UserId) -> Result<ShellSnapshot, ServiceError> {
        let state = self.accounts.load()?;
        let user = state
            .accounts()
            .get(user_id)
            .filter(|a| a.can_authenticate())
            .ok_or_else(|| std::io::Error::other("account unavailable"))?;
        let platform = self.platform.load()?;
        let count = platform.as_ref().map_or(0, |p| p.installed_apps().len());
        let now = UnixTimestamp::now()?
            .as_seconds()
            .checked_mul(1000)
            .ok_or_else(|| std::io::Error::other("invalid clock"))?;
        let revision = sha256_hex(&serde_json::to_vec(&(
            user_id.to_string(),
            user.display_name(),
            &self.locale,
            &self.build_id,
            count,
        ))?);
        Ok(ShellSnapshot::new(
            &self.build_id,
            revision,
            ShellUser::new(user.display_name(), &self.locale)?,
            stock_theme()?,
            // No system health assessment exists yet: never claim the device is secure.
            ShellSystemStatus::new(
                SystemProtectionStatus::Attention,
                u32::try_from(count)?,
                now,
                None,
            )?,
            vec![],
        )?)
    }
}
impl ShellSnapshotProvider for PersistentShellSnapshots {
    type Error = std::io::Error;
    fn load_for_user(&self, subject: &ShellSnapshotSubject) -> Result<ShellSnapshot, Self::Error> {
        self.for_user(subject.user_id())
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
    Ok(Arc::new(PlatformShellBackend::new(
        auth,
        PersistentShellSnapshots::open(accounts, platform, build_id, locale)?,
    )))
}

pub fn stock_stylesheet() -> (String, String) {
    let css = ResolvedTheme::stock().compile_css();
    let path = format!("/shell/themes/sha256-{}.css", sha256_hex(css.as_bytes()));
    (path, css)
}
fn stock_theme() -> Result<ShellTheme, rumahl_ui_contracts::ShellSnapshotError> {
    ShellTheme::new(stock_stylesheet().0, ResolvedTheme::stock().window_chrome())
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
