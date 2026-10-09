//! Installation metadata and explicit launch grants come from the platform snapshot.
//! Published web roots are immutable, installation-specific directories.
use rumahl_core::*;
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteSnapshotRepository};
use rumahl_platform_web::{
    AppAccessError, AppAsset, AppData, AppDataEntry, AppProvider, AppSettingInfo,
    AppSettingOptionInfo, BrowserDelivery, CapabilityOutcome, CapabilityResource, CapabilityResult,
    CatalogApp, ImportedApp, PackageUploadFile, RuntimeState, ShellIdentity,
};
use rustix::fs::{Dir, FileType, Mode, OFlags, fstat, open, openat};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const APP_LAUNCH_PERMISSION: &str = "rumahl.apps.launch";
pub fn app_launch_resource(installation: InstallationId) -> ResourceRef {
    ResourceRef::new(
        ResourceNamespace::parse("rumahl.apps").unwrap(),
        ResourceKind::parse("installation").unwrap(),
        ResourceKey::parse(installation.to_string()).unwrap(),
    )
}
/// The permission ids an installed app's manifest declares, sorted, deduped and
/// bounded. The shell gates each app <-> OS bridge method against this list.
fn app_capabilities(app: &InstalledAppSnapshot) -> Vec<String> {
    let mut ids: Vec<String> = app
        .manifest()
        .permission_requests()
        .iter()
        .map(|request| request.permission().as_str().to_owned())
        .collect();
    ids.sort();
    ids.dedup();
    ids.truncate(64);
    ids
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
    /// Apps data volume root, one directory per app id. `None` disables app-data browsing.
    data_root: Option<PathBuf>,
    /// Runtime adapters for status/start/stop. `None` fails closed.
    runtime: Option<Arc<rumahl_core::RuntimeAdapterRegistry>>,
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
            data_root: None,
            runtime: None,
        })
    }

    /// Enables runtime status/start/stop through the registered adapters.
    pub fn with_runtime_adapters(
        mut self,
        runtime: Arc<rumahl_core::RuntimeAdapterRegistry>,
    ) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Enables read-only browsing of each app's private data directory.
    pub fn with_data_root(mut self, data_root: PathBuf) -> Result<Self, crate::ServiceError> {
        if !data_root.is_absolute() {
            return Err(std::io::Error::other("app data root must be absolute").into());
        }
        self.data_root = Some(data_root);
        Ok(self)
    }
    /// Recovers platform state and builds a runtime controller when runtime
    /// adapters are configured. Fails closed otherwise.
    fn runtime_control(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<RuntimeController, AppAccessError> {
        let adapters = self.runtime.clone().ok_or(AppAccessError::Unavailable)?;
        self.app(identity, installation)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?
            .ok_or(AppAccessError::Unavailable)?;
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();
        PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .map_err(|_| AppAccessError::Unavailable)?;
        Ok(RuntimeController::new(Arc::new(state), adapters))
    }

    fn apps(&self, identity: ShellIdentity) -> Result<Vec<InstalledAppSnapshot>, AppAccessError> {
        self.authenticate(identity)?;
        match self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?
        {
            None => Ok(vec![]),
            Some(snapshot) => authorized_apps(&snapshot, identity),
        }
    }
    /// Confirms the account can authenticate and the session is live.
    fn authenticate(&self, identity: ShellIdentity) -> Result<(), AppAccessError> {
        let accounts = self
            .accounts
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let now = UnixTimestamp::now().map_err(|_| AppAccessError::Unavailable)?;
        if accounts
            .accounts()
            .get(&identity.user_id)
            .is_some_and(|a| a.can_authenticate())
            && accounts
                .sessions()
                .get(&identity.session_id)
                .is_some_and(|s| s.user_id() == &identity.user_id && s.is_active_at(now))
        {
            Ok(())
        } else {
            Err(AppAccessError::Denied)
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

    fn content_type(path: &str) -> Option<&'static str> {
        match Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("html") => Some("text/html; charset=utf-8"),
            Some("js" | "mjs") => Some("text/javascript; charset=utf-8"),
            Some("css") => Some("text/css; charset=utf-8"),
            Some("json" | "jsonl") => Some("application/json"),
            Some(
                "txt" | "log" | "md" | "env" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf",
            ) => Some("text/plain; charset=utf-8"),
            Some("csv") => Some("text/csv; charset=utf-8"),
            Some("xml") => Some("application/xml"),
            Some("png") => Some("image/png"),
            Some("jpg" | "jpeg") => Some("image/jpeg"),
            Some("svg") => Some("image/svg+xml"),
            Some("webp") => Some("image/webp"),
            Some("gif") => Some("image/gif"),
            Some("pdf") => Some("application/pdf"),
            Some("sqlite" | "sqlite3" | "db") => Some("application/vnd.sqlite3"),
            _ => None,
        }
    }

    fn list_app_directory(fd: &rustix::fd::OwnedFd) -> Result<AppData, AppAccessError> {
        let directory = Dir::read_from(fd).map_err(|_| AppAccessError::Denied)?;
        let mut entries = Vec::new();
        for entry in directory {
            let entry = entry.map_err(|_| AppAccessError::Unavailable)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "." || name == ".." {
                continue;
            }
            if entries.len() >= 1024 {
                break;
            }
            let is_directory = entry.file_type() == FileType::Directory;
            let size = if is_directory {
                0
            } else {
                match openat(
                    fd,
                    name.as_str(),
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                    Mode::empty(),
                ) {
                    Ok(child) => match fstat(&child) {
                        Ok(stat)
                            if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile =>
                        {
                            u64::try_from(stat.st_size.max(0)).unwrap_or(0)
                        }
                        _ => continue,
                    },
                    Err(_) => continue,
                }
            };
            entries.push(AppDataEntry {
                name,
                directory: is_directory,
                size,
            });
        }
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(AppData::Directory(entries))
    }

    fn read_app_data(&self, app_id: &str, path: &str) -> Result<AppData, AppAccessError> {
        let Some(root) = self.data_root.as_ref() else {
            return Err(AppAccessError::Unavailable);
        };
        read_app_data_at(root, app_id, path)
    }
}

fn read_app_data_at(root: &Path, app_id: &str, path: &str) -> Result<AppData, AppAccessError> {
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY;
    let root_fd = open(root, flags, Mode::empty()).map_err(|_| AppAccessError::Denied)?;
    let mut directory =
        openat(&root_fd, app_id, flags, Mode::empty()).map_err(|_| AppAccessError::Denied)?;
    let mut components = path.split('/').filter(|component| !component.is_empty());
    let mut component = components.next();
    while let Some(part) = component {
        if part == "." || part == ".." || part.contains('\\') || part.contains('\0') {
            return Err(AppAccessError::Denied);
        }
        let following = components.next();
        if let Some(next) = following {
            directory = openat(&directory, part, flags, Mode::empty())
                .map_err(|_| AppAccessError::Denied)?;
            component = Some(next);
            continue;
        }
        if let Ok(final_directory) = openat(&directory, part, flags, Mode::empty()) {
            return PersistentApps::list_app_directory(&final_directory);
        }
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
        File::from(fd)
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AppAccessError::Unavailable)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(AppAccessError::Denied);
        }
        let content_type = PersistentApps::content_type(part).unwrap_or("application/octet-stream");
        return Ok(AppData::File {
            bytes,
            content_type,
        });
    }
    PersistentApps::list_app_directory(&directory)
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
                capabilities: app_capabilities(app),
                lifecycle: app.manifest().lifecycle().as_str().into(),
                connectors: app
                    .manifest()
                    .connectors()
                    .iter()
                    .map(|connector| connector.target().as_str().to_owned())
                    .collect(),
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
    fn settings(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<Vec<AppSettingInfo>, AppAccessError> {
        let app = self.app(identity, installation)?;
        Ok(app
            .manifest()
            .settings()
            .iter()
            .map(|setting| AppSettingInfo {
                key: setting.key().as_str().to_owned(),
                title: setting.title().to_owned(),
                description: setting.description().map(str::to_owned),
                kind: setting.kind().as_str().to_owned(),
                required: setting.is_required(),
                options: match setting.kind() {
                    AppSettingKind::Select { options } => options
                        .iter()
                        .map(|option| AppSettingOptionInfo {
                            value: option.value().to_owned(),
                            label: option.label().to_owned(),
                        })
                        .collect(),
                    _ => Vec::new(),
                },
            })
            .collect())
    }
    fn data(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppData, AppAccessError> {
        let app = self.app(identity, installation)?;
        self.read_app_data(app.identity().app_id().as_str(), path)
    }
    fn runtime_status(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        runtime_state(self.runtime_control(identity, installation)?.status(&installation))
    }
    fn start_runtime(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        runtime_state(self.runtime_control(identity, installation)?.start(&installation))
    }
    fn stop_runtime(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        runtime_state(self.runtime_control(identity, installation)?.stop(&installation))
    }
    fn import_package(
        &self,
        _identity: ShellIdentity,
        files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        let state_dir = self.root.parent().ok_or(AppAccessError::Unavailable)?;
        let staging = state_dir.join("import-staging").join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let result = write_staging(&staging, &files).and_then(|()| {
            crate::install::install_package_dir(state_dir, &staging)
                .map_err(|_| AppAccessError::Unavailable)
        });
        let _ = std::fs::remove_dir_all(&staging);
        let info = result?;
        Ok(ImportedApp {
            id: info.app_id,
            installation_id: info.installation_id,
            title: info.title,
            version: info.version,
        })
    }
    fn invoke_capability(
        &self,
        identity: ShellIdentity,
        capability: String,
        resource: Option<CapabilityResource>,
    ) -> Result<CapabilityResult, AppAccessError> {
        self.authenticate(identity)?;
        let adapters = self.runtime.clone().ok_or(AppAccessError::Unavailable)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?
            .ok_or(AppAccessError::Unavailable)?;
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();
        PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .map_err(|_| AppAccessError::Unavailable)?;
        let (capabilities, access) =
            build_capability_registries(&state).map_err(|_| AppAccessError::Unavailable)?;
        let capability = CapabilityId::parse(capability).map_err(|_| AppAccessError::Denied)?;
        let Some(provider) = capabilities
            .providers_for(&capability)
            .into_iter()
            .next()
            .cloned()
        else {
            return Err(AppAccessError::Denied);
        };
        let resource = match resource {
            None => None,
            Some(resource) => Some(ResourceRef::new(
                ResourceNamespace::parse(resource.namespace).map_err(|_| AppAccessError::Denied)?,
                ResourceKind::parse(resource.kind).map_err(|_| AppAccessError::Denied)?,
                ResourceKey::parse(resource.key).map_err(|_| AppAccessError::Denied)?,
            )),
        };
        let provider_identity = provider.identity().clone();
        let Identity::App(provider_app) = &provider_identity else {
            return Err(AppAccessError::Denied);
        };
        let provider_installation = *provider_app.installation_id();
        let web_provider = state
            .installed_apps()
            .get_by_installation_id(&provider_installation)
            .is_some_and(|app| app.manifest().runtime().kind() == RuntimeKind::Web);

        let context =
            OperationContext::for_user(UserIdentity::new(identity.user_id), identity.session_id);
        let invocation = CapabilityInvocation::new(context, provider);

        // A web provider runs only in the shell's iframe and has no live channel,
        // so the shell delivers the invocation to the frame.
        if web_provider {
            let decision = CapabilityInvoker::new()
                .authorize(&invocation, resource, &capabilities, &access, grants.grants())
                .map_err(|_| AppAccessError::Denied)?;
            return Ok(match decision {
                AuthorizationDecision::Allow => CapabilityResult {
                    outcome: CapabilityOutcome::Invoked,
                    result: None,
                    browser: Some(BrowserDelivery {
                        app_id: provider_app.app_id().as_str().to_owned(),
                        installation_id: provider_installation.to_string(),
                    }),
                },
                _ => CapabilityResult {
                    outcome: CapabilityOutcome::Denied,
                    result: None,
                    browser: None,
                },
            });
        }

        match CapabilityInvoker::new().invoke(
            &invocation,
            resource,
            &state,
            &capabilities,
            &access,
            grants.grants(),
            &adapters,
        ) {
            Ok(CapabilityInvocationOutcome::Invoked(result)) => Ok(CapabilityResult {
                outcome: CapabilityOutcome::Invoked,
                result: (!result.is_empty()).then(|| result.into_payload()),
                browser: None,
            }),
            Ok(CapabilityInvocationOutcome::NotAuthorized(_)) => Ok(CapabilityResult {
                outcome: CapabilityOutcome::Denied,
                result: None,
                browser: None,
            }),
            Err(_) => Err(AppAccessError::Denied),
        }
    }
}

/// Writes uploaded package files into a fresh owner-only staging directory.
fn write_staging(dir: &Path, files: &[PackageUploadFile]) -> Result<(), AppAccessError> {
    std::fs::create_dir_all(dir).map_err(|_| AppAccessError::Unavailable)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| AppAccessError::Unavailable)?;
    for file in files {
        let target = dir.join(&file.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AppAccessError::Unavailable)?;
        }
        std::fs::write(&target, &file.bytes).map_err(|_| AppAccessError::Unavailable)?;
    }
    Ok(())
}

fn runtime_state(result: Result<RuntimeStatus, RuntimeRoutingError>) -> Result<RuntimeState, AppAccessError> {
    match result {
        Ok(RuntimeStatus::Running | RuntimeStatus::Starting) => Ok(RuntimeState::Running),
        Ok(RuntimeStatus::Stopped | RuntimeStatus::Stopping | RuntimeStatus::Failed) => {
            Ok(RuntimeState::Stopped)
        }
        Err(_) => Err(AppAccessError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn setup() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("com.rumahl.notes");
        std::fs::create_dir_all(app.join("docs")).unwrap();
        std::fs::write(app.join("notes.txt"), b"hello").unwrap();
        std::fs::write(app.join("docs/readme.md"), b"# readme").unwrap();
        temp
    }

    #[test]
    fn lists_and_reads_only_inside_the_app_directory() {
        let temp = setup();
        let AppData::Directory(entries) =
            read_app_data_at(temp.path(), "com.rumahl.notes", "").unwrap()
        else {
            panic!("expected a directory");
        };
        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "notes.txt" && !entry.directory && entry.size == 5)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "docs" && entry.directory)
        );

        let AppData::File { bytes, .. } =
            read_app_data_at(temp.path(), "com.rumahl.notes", "docs/readme.md").unwrap()
        else {
            panic!("expected a file");
        };
        assert_eq!(bytes, b"# readme");
    }

    #[test]
    fn rejects_traversal_symlinks_and_other_apps() {
        let temp = setup();
        std::fs::write(temp.path().join("secret.txt"), b"secret").unwrap();
        let app = temp.path().join("com.rumahl.notes");
        symlink(temp.path().join("secret.txt"), app.join("escape")).unwrap();
        let _ = symlink("/etc", app.join("etc"));

        assert!(read_app_data_at(temp.path(), "com.rumahl.notes", "../secret.txt").is_err());
        assert!(read_app_data_at(temp.path(), "com.rumahl.notes", "escape").is_err());
        assert!(read_app_data_at(temp.path(), "com.rumahl.notes", "etc/passwd").is_err());
        assert!(read_app_data_at(temp.path(), "com.rumahl.other", "").is_err());
    }
}
