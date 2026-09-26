//! Installation metadata and explicit launch grants come from the platform snapshot.
//! Published web roots are immutable, installation-specific directories.
use rumahl_core::*;
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteSnapshotRepository};
use rumahl_platform_web::{AppAccessError, AppAsset, AppProvider, CatalogApp, ShellIdentity};
use rustix::fs::{FileType, Mode, OFlags, fstat, open, openat};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const APP_LAUNCH_PERMISSION: &str = "rumahl.apps.launch";
pub fn app_launch_resource(installation: InstallationId) -> ResourceRef {
    ResourceRef::new(
        ResourceNamespace::parse("rumahl.apps").unwrap(),
        ResourceKind::parse("installation").unwrap(),
        ResourceKey::parse(installation.to_string()).unwrap(),
    )
}
pub fn authorized_apps(
    snapshot: &PlatformSnapshot,
    identity: ShellIdentity,
) -> Result<Vec<InstalledAppSnapshot>, AppAccessError> {
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    PlatformRecovery::new()
        .recover(snapshot, &mut state, &mut grants)
        .map_err(|_| AppAccessError::Unavailable)?;
    let context =
        OperationContext::for_user(UserIdentity::new(identity.user_id), identity.session_id);
    let permission = PermissionId::parse(APP_LAUNCH_PERMISSION).unwrap();
    let mut result: Vec<_> = snapshot
        .installed_apps()
        .iter()
        .filter(|app| {
            let request = AuthorizationRequest::new(
                context.clone(),
                permission.clone(),
                Some(app_launch_resource(*app.installation_id())),
            );
            AuthorizationEngine::new().authorize(&request, grants.grants())
                == AuthorizationDecision::Allow
        })
        .cloned()
        .collect();
    result.sort_by(|a, b| {
        a.identity()
            .app_id()
            .as_str()
            .cmp(b.identity().app_id().as_str())
    });
    if result.len() > 256 {
        return Err(AppAccessError::Unavailable);
    }
    Ok(result)
}
pub struct PersistentApps {
    accounts: SqliteAccountStateRepository,
    platform: SqliteSnapshotRepository,
    root: PathBuf,
}
impl PersistentApps {
    pub fn open(
        accounts: &Path,
        platform: &Path,
        root: PathBuf,
    ) -> Result<Self, crate::ServiceError> {
        if !root.is_absolute() {
            return Err(std::io::Error::other("app assets root must be absolute").into());
        }
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(accounts)?,
            platform: SqliteSnapshotRepository::open(platform)?,
            root,
        })
    }
    fn apps(&self, identity: ShellIdentity) -> Result<Vec<InstalledAppSnapshot>, AppAccessError> {
        let accounts = self
            .accounts
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let now = UnixTimestamp::now().map_err(|_| AppAccessError::Unavailable)?;
        if !accounts
            .accounts()
            .get(&identity.user_id)
            .is_some_and(|a| a.can_authenticate())
            || !accounts
                .sessions()
                .get(&identity.session_id)
                .is_some_and(|s| s.user_id() == &identity.user_id && s.is_active_at(now))
        {
            return Err(AppAccessError::Denied);
        }
        match self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?
        {
            None => Ok(vec![]),
            Some(snapshot) => authorized_apps(&snapshot, identity),
        }
    }
    fn app(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<InstalledAppSnapshot, AppAccessError> {
        self.apps(identity)?
            .into_iter()
            .find(|app| *app.installation_id() == installation)
            .ok_or(AppAccessError::Denied)
    }
    fn web_entry(app: &InstalledAppSnapshot) -> Option<&str> {
        if app.manifest().runtime().kind() != RuntimeKind::Web {
            return None;
        }
        // No implicit choice among multiple entrypoints; the public web entry is main.
        let main = RuntimeEntrypointId::parse("main").ok()?;
        app.manifest()
            .runtime()
            .entrypoint(&main)?
            .target()
            .package_path()
            .map(PackagePath::as_str)
    }
    fn read_asset(
        &self,
        installation: InstallationId,
        path: &str,
        contents: bool,
    ) -> Result<AppAsset, AppAccessError> {
        PackagePath::parse(path).map_err(|_| AppAccessError::Denied)?;
        let content_type = match Path::new(path).extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("js" | "mjs") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("json") => "application/json",
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("svg") => "image/svg+xml",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("ico") => "image/x-icon",
            Some("woff") => "font/woff",
            Some("woff2") => "font/woff2",
            _ => return Err(AppAccessError::Denied),
        };
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY;
        let root = open(&self.root, flags, Mode::empty()).map_err(|_| AppAccessError::Denied)?;
        let mut directory = openat(&root, installation.to_string(), flags, Mode::empty())
            .map_err(|_| AppAccessError::Denied)?;
        let mut parts = path.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                directory = openat(&directory, part, flags, Mode::empty())
                    .map_err(|_| AppAccessError::Denied)?;
            } else {
                let fd = openat(
                    &directory,
                    part,
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                    Mode::empty(),
                )
                .map_err(|_| AppAccessError::Denied)?;
                let stat = fstat(&fd).map_err(|_| AppAccessError::Denied)?;
                if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
                    || stat.st_size > 2 * 1024 * 1024
                {
                    return Err(AppAccessError::Denied);
                }
                let mut bytes = Vec::new();
                if contents {
                    File::from(fd)
                        .take(2 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| AppAccessError::Unavailable)?;
                }
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(AppAccessError::Denied);
                }
                return Ok(AppAsset {
                    bytes,
                    content_type,
                });
            }
        }
        Err(AppAccessError::Denied)
    }
}
impl AppProvider for PersistentApps {
    fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
        Ok(self
            .apps(identity)?
            .iter()
            .map(|app| CatalogApp {
                id: app.identity().app_id().as_str().into(),
                installation_id: *app.installation_id(),
                title: app.manifest().display_name().into(),
                version: app.manifest().version().to_string(),
                launchable: Self::web_entry(app).is_some_and(|path| {
                    self.read_asset(*app.installation_id(), path, false).is_ok()
                }),
            })
            .collect())
    }
    fn entrypoint(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<String, AppAccessError> {
        let app = self.app(identity, installation)?;
        let path = Self::web_entry(&app).ok_or(AppAccessError::Denied)?;
        self.read_asset(installation, path, false)?;
        Ok(path.into())
    }
    fn asset(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppAsset, AppAccessError> {
        let app = self.app(identity, installation)?;
        let entry = Self::web_entry(&app).ok_or(AppAccessError::Denied)?;
        // Only the web entrypoint's directory is published, not package secrets or artifacts.
        let directory = entry
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory);
        if !directory.is_empty() && !path.starts_with(&format!("{directory}/")) {
            return Err(AppAccessError::Denied);
        }
        self.read_asset(installation, path, true)
    }
}
