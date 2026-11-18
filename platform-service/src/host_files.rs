//! Real host roots the file explorer may browse, backed by the OS layout.
//!
//! `/apps` and the protected `/rumahl` tree are read-only. The user's own home
//! (`/home/<username>`) is the only writable host area: the shell never writes
//! into installed applications or the system tree.
use rumahl_core::{AccountStateRepository, UserId};
use rumahl_persistence_sqlite::SqliteAccountStateRepository;
use rumahl_platform_buildroot::FilesystemLayout;
use rumahl_platform_web::{HostArea, HostEntry, HostError, HostFiles};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_ENTRIES: usize = 2000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// One host write; the web layer enforces the same bound.
const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 255;
const FILE_MODE: u32 = 0o600;
const DIRECTORY_MODE: u32 = 0o700;

/// Reads `/apps` and the protected `/rumahl` tree and owns the writable home
/// area. The roots come from `RUMAHL_APPS_ROOT` / `RUMAHL_SYSTEM_ROOT` /
/// `RUMAHL_HOME_ROOT`, falling back to the layout.
pub struct LocalHostFiles {
    apps_root: PathBuf,
    system_root: PathBuf,
    home_root: PathBuf,
    /// Accounts database for mapping the calling user to its home directory.
    /// `None` makes the home area unavailable.
    accounts: Option<PathBuf>,
}

impl LocalHostFiles {
    pub fn new(apps_root: PathBuf, system_root: PathBuf) -> Self {
        let layout = FilesystemLayout::system();
        Self {
            apps_root,
            system_root,
            home_root: layout.home_root().to_path_buf(),
            accounts: None,
        }
    }

    /// Enables the writable home area: `/home/<username>` for account holders.
    pub fn with_homes(mut self, home_root: PathBuf, accounts: PathBuf) -> Self {
        self.home_root = home_root;
        self.accounts = Some(accounts);
        self
    }

    pub fn from_environment(accounts: &Path) -> Self {
        let layout = FilesystemLayout::system();
        let apps_root = std::env::var_os("RUMAHL_APPS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| layout.apps_root().to_path_buf());
        let system_root = std::env::var_os("RUMAHL_SYSTEM_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| layout.system_root().to_path_buf());
        let home_root = std::env::var_os("RUMAHL_HOME_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| layout.home_root().to_path_buf());
        Self {
            apps_root,
            system_root,
            home_root,
            accounts: Some(accounts.to_path_buf()),
        }
    }

    pub fn home_root(&self) -> &Path {
        &self.home_root
    }

    /// The caller's account name. Unknown users resolve to nothing: no area
    /// ever inherits another user's name.
    fn account_name(&self, user: UserId) -> Result<String, HostError> {
        let Some(accounts) = self.accounts.as_ref() else {
            return Err(HostError::Denied);
        };
        let state = SqliteAccountStateRepository::open(accounts)
            .and_then(|repository| repository.load())
            .map_err(|_| HostError::Unavailable)?;
        state
            .accounts()
            .get(&user)
            .map(|account| account.username().as_str().to_owned())
            .ok_or(HostError::Denied)
    }

    /// The account's home directory. Unknown users and missing directories
    /// resolve to nothing: no area ever inherits another user's path.
    fn home_of(&self, user: UserId) -> Result<PathBuf, HostError> {
        let account = self.account_name(user)?;
        let root = self
            .home_root
            .canonicalize()
            .map_err(|_| HostError::Missing)?;
        let home = root.join(&account);
        let canonical = home.canonicalize().map_err(|_| HostError::Missing)?;
        if !canonical.starts_with(&root) {
            return Err(HostError::Denied);
        }
        Ok(canonical)
    }

    fn root(&self, area: HostArea, user: UserId) -> Result<PathBuf, HostError> {
        match area {
            HostArea::Apps => self
                .apps_root
                .canonicalize()
                .map_err(|_| HostError::Missing),
            HostArea::System => self
                .system_root
                .canonicalize()
                .map_err(|_| HostError::Missing),
            HostArea::Home => self.home_of(user),
        }
    }

    /// Resolves a relative, client-supplied path inside the area root. Rejects
    /// traversal and verifies the canonical target stays under the real root.
    fn resolve(&self, area: HostArea, relative: &str, user: UserId) -> Result<PathBuf, HostError> {
        if relative.contains('\0') || relative.starts_with('/') {
            return Err(HostError::Invalid);
        }
        let root = self.root(area, user)?;
        let mut target = root.clone();
        for segment in relative.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." || segment.contains('\\') {
                return Err(HostError::Invalid);
            }
            target.push(segment);
        }
        let canonical = target.canonicalize().map_err(|_| HostError::Missing)?;
        if !canonical.starts_with(&root) {
            return Err(HostError::Denied);
        }
        Ok(canonical)
    }

    /// Resolves the parent of a not-yet-existing entry and validates the name.
    /// The target itself is never followed, so a symlink can neither be written
    /// through nor removed.
    fn resolve_parent(
        &self,
        area: HostArea,
        relative: &str,
        user: UserId,
    ) -> Result<(PathBuf, String), HostError> {
        let (parent, name) = relative.rsplit_once('/').unwrap_or(("", relative));
        let name = name.to_owned();
        validate_name(&name)?;
        let directory = self.resolve(area, parent, user)?;
        let meta = std::fs::symlink_metadata(&directory).map_err(|_| HostError::Missing)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        Ok((directory, name))
    }

    /// Write access is the home area's alone; anything else is refused before
    /// touching the filesystem.
    fn writable(&self, area: HostArea) -> Result<(), HostError> {
        if area == HostArea::Home {
            Ok(())
        } else {
            Err(HostError::Denied)
        }
    }
}

fn validate_name(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err(HostError::Invalid);
    }
    Ok(())
}

impl HostFiles for LocalHostFiles {
    fn list(&self, area: HostArea, path: &str, user: UserId) -> Result<Vec<HostEntry>, HostError> {
        let directory = self.resolve(area, path, user)?;
        let meta = std::fs::symlink_metadata(&directory).map_err(|_| HostError::Missing)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        let mut entries = Vec::new();
        let reader = std::fs::read_dir(&directory).map_err(|_| HostError::Unavailable)?;
        for entry in reader {
            let entry = entry.map_err(|_| HostError::Unavailable)?;
            if entries.len() >= MAX_ENTRIES {
                break;
            }
            let meta = match std::fs::symlink_metadata(entry.path()) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let directory = meta.is_dir();
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            entries.push(HostEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                directory,
                size: if directory { 0 } else { meta.len() },
                modified,
            });
        }
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(entries)
    }

    fn read(
        &self,
        area: HostArea,
        path: &str,
        user: UserId,
    ) -> Result<(String, Vec<u8>), HostError> {
        let file = self.resolve(area, path, user)?;
        let meta = std::fs::symlink_metadata(&file).map_err(|_| HostError::Missing)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(HostError::Limit);
        }
        let data = std::fs::read(&file).map_err(|_| HostError::Unavailable)?;
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_owned());
        Ok((name, data))
    }

    fn username(&self, user: UserId) -> Result<String, HostError> {
        self.account_name(user)
    }

    fn stat(&self, area: HostArea, path: &str, user: UserId) -> Result<(String, bool), HostError> {
        if path.is_empty() {
            let root = self.root(area, user)?;
            return Ok((String::new(), root.is_dir()));
        }
        let target = self.resolve(area, path, user)?;
        let meta = std::fs::symlink_metadata(&target).map_err(|_| HostError::Missing)?;
        if meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        let name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((name, meta.is_dir()))
    }

    fn write(
        &self,
        area: HostArea,
        path: &str,
        user: UserId,
        data: &[u8],
    ) -> Result<(), HostError> {
        self.writable(area)?;
        if data.len() > MAX_WRITE_BYTES {
            return Err(HostError::Limit);
        }
        let (directory, name) = self.resolve_parent(area, path, user)?;
        let target = directory.join(&name);
        if let Ok(meta) = std::fs::symlink_metadata(&target)
            && (meta.file_type().is_symlink() || !meta.is_file())
        {
            return Err(HostError::Invalid);
        }
        // Write-through to a fresh temporary file, then move into place: a
        // reader never sees a half-written document.
        let temporary = directory.join(format!(".{name}.tmp-{}", UserId::new()));
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(FILE_MODE)
                .open(&temporary)
                .map_err(|_| HostError::Unavailable)?;
            file.write_all(data).map_err(|_| HostError::Unavailable)?;
            file.sync_all().map_err(|_| HostError::Unavailable)?;
        }
        std::fs::rename(&temporary, &target).map_err(|_| HostError::Unavailable)
    }

    fn create(
        &self,
        area: HostArea,
        parent: &str,
        name: &str,
        directory: bool,
        user: UserId,
    ) -> Result<(), HostError> {
        self.writable(area)?;
        validate_name(name)?;
        let directory_path = self.resolve(area, parent, user)?;
        let meta = std::fs::symlink_metadata(&directory_path).map_err(|_| HostError::Missing)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        let target = directory_path.join(name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(HostError::Invalid);
        }
        if directory {
            std::fs::create_dir(&target).map_err(|_| HostError::Unavailable)?;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(DIRECTORY_MODE))
                .map_err(|_| HostError::Unavailable)
        } else {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(FILE_MODE)
                .open(&target)
                .map(|_| ())
                .map_err(|_| HostError::Unavailable)
        }
    }

    fn remove(&self, area: HostArea, path: &str, user: UserId) -> Result<(), HostError> {
        self.writable(area)?;
        let (directory, name) = self.resolve_parent(area, path, user)?;
        let target = directory.join(&name);
        let meta = std::fs::symlink_metadata(&target).map_err(|_| HostError::Missing)?;
        if meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        if meta.is_dir() {
            // Only empty directories: deletion never recurses on host paths.
            std::fs::remove_dir(&target).map_err(|_| HostError::Invalid)
        } else {
            std::fs::remove_file(&target).map_err(|_| HostError::Unavailable)
        }
    }

    fn rename(
        &self,
        area: HostArea,
        path: &str,
        name: &str,
        user: UserId,
    ) -> Result<(), HostError> {
        self.writable(area)?;
        validate_name(name)?;
        let (directory, current) = self.resolve_parent(area, path, user)?;
        let source = directory.join(&current);
        let meta = std::fs::symlink_metadata(&source).map_err(|_| HostError::Missing)?;
        if meta.file_type().is_symlink() {
            return Err(HostError::Invalid);
        }
        let target = directory.join(name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(HostError::Invalid);
        }
        std::fs::rename(&source, &target).map_err(|_| HostError::Unavailable)
    }
}

use std::os::unix::fs::PermissionsExt;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::LocalPasswordBlocklist;

    fn roots() -> (tempfile::TempDir, LocalHostFiles) {
        let temp = tempfile::tempdir().unwrap();
        let apps = temp.path().join("apps");
        let system = temp.path().join("system");
        std::fs::create_dir_all(apps.join("com.example.notes/data")).unwrap();
        std::fs::write(apps.join("com.example.notes/data/notes.txt"), b"hello").unwrap();
        std::fs::create_dir_all(&system).unwrap();
        (temp, LocalHostFiles::new(apps, system))
    }

    fn account(temp: &Path, username: &str) -> (UserId, PathBuf) {
        let accounts = temp.join("accounts.sqlite");
        crate::provision(
            &accounts,
            username,
            "Test",
            rumahl_account_auth::SessionToken::generate()
                .unwrap()
                .encode()
                .to_string(),
            LocalPasswordBlocklist::default(),
        )
        .unwrap();
        let state = SqliteAccountStateRepository::open(&accounts)
            .unwrap()
            .load()
            .unwrap();
        let user_id = *state
            .accounts()
            .accounts()
            .iter()
            .find(|user| user.username().as_str() == username)
            .unwrap()
            .user_id();
        (user_id, accounts)
    }

    fn with_home(temp: &Path, username: &str) -> (UserId, LocalHostFiles) {
        let (user, accounts) = account(temp, username);
        let home_root = temp.join("home");
        let home = home_root.join(username);
        std::fs::create_dir_all(home.join("Documents")).unwrap();
        std::fs::write(home.join("Documents/notes.txt"), b"mine").unwrap();
        std::fs::create_dir_all(&home_root).unwrap();
        let host = LocalHostFiles::new(temp.join("apps"), temp.join("system"))
            .with_homes(home_root, accounts);
        (user, host)
    }

    #[test]
    fn lists_directories_and_files() {
        let (_temp, host) = roots();
        let user = UserId::new();
        let entries = host
            .list(HostArea::Apps, "com.example.notes/data", user)
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "notes.txt");
        assert!(!entries[0].directory);
        assert_eq!(entries[0].size, 5);
    }

    #[test]
    fn reads_files_and_rejects_traversal_and_symlinks() {
        let (temp, host) = roots();
        let user = UserId::new();
        let (name, data) = host
            .read(HostArea::Apps, "com.example.notes/data/notes.txt", user)
            .unwrap();
        assert_eq!(name, "notes.txt");
        assert_eq!(data, b"hello");

        assert_eq!(
            host.list(HostArea::Apps, "../system", user).unwrap_err(),
            HostError::Invalid
        );
        assert_eq!(
            host.read(HostArea::Apps, "/etc/passwd", user).unwrap_err(),
            HostError::Invalid
        );

        std::os::unix::fs::symlink(temp.path().join("system"), host.apps_root.join("escape"))
            .unwrap();
        assert!(host.list(HostArea::Apps, "escape", user).is_err());
    }

    #[test]
    fn the_home_area_belongs_to_the_calling_user_only() {
        let temp = tempfile::tempdir().unwrap();
        let (user, host) = with_home(temp.path(), "alice");
        let (other, _) = account(temp.path(), "bob");

        let entries = host.list(HostArea::Home, "", user).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Documents");

        // Bob has an account but no home directory yet: the area is absent,
        // never alice's.
        assert_eq!(
            host.list(HostArea::Home, "", other).unwrap_err(),
            HostError::Missing
        );
        // Unknown users get nothing at all.
        assert_eq!(
            host.list(HostArea::Home, "", UserId::new()).unwrap_err(),
            HostError::Denied
        );
    }

    #[test]
    fn only_the_home_area_is_writable() {
        let temp = tempfile::tempdir().unwrap();
        let (user, host) = with_home(temp.path(), "alice");

        assert_eq!(
            host.write(
                HostArea::Apps,
                "com.example.notes/data/notes.txt",
                user,
                b"x"
            )
            .unwrap_err(),
            HostError::Denied
        );
        assert_eq!(
            host.create(HostArea::System, "", "evil", false, user)
                .unwrap_err(),
            HostError::Denied
        );
        assert_eq!(
            host.remove(HostArea::Apps, "com.example.notes/data/notes.txt", user)
                .unwrap_err(),
            HostError::Denied
        );
        assert_eq!(
            host.rename(HostArea::System, "x", "y", user).unwrap_err(),
            HostError::Denied
        );
    }

    #[test]
    fn home_writes_are_atomic_contained_and_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let (user, host) = with_home(temp.path(), "alice");

        host.write(HostArea::Home, "Documents/notes.txt", user, b"rewritten")
            .unwrap();
        let (_, data) = host
            .read(HostArea::Home, "Documents/notes.txt", user)
            .unwrap();
        assert_eq!(data, b"rewritten");

        // No traversal, no writing through symlinks.
        assert_eq!(
            host.write(HostArea::Home, "../escape.txt", user, b"x")
                .unwrap_err(),
            HostError::Invalid
        );
        let outside = temp.path().join("outside.txt");
        std::fs::write(&outside, b"secret").unwrap();
        std::os::unix::fs::symlink(&outside, host.home_root().join("alice/link.txt")).unwrap();
        assert_eq!(
            host.write(HostArea::Home, "link.txt", user, b"x")
                .unwrap_err(),
            HostError::Invalid
        );
        assert_eq!(std::fs::read(&outside).unwrap(), b"secret");
    }

    #[test]
    fn home_entries_can_be_created_renamed_and_removed() {
        let temp = tempfile::tempdir().unwrap();
        let (user, host) = with_home(temp.path(), "alice");

        host.create(HostArea::Home, "", "Projects", true, user)
            .unwrap();
        host.create(HostArea::Home, "Projects", "todo.txt", false, user)
            .unwrap();
        let entries = host.list(HostArea::Home, "Projects", user).unwrap();
        assert_eq!(entries.len(), 1);

        host.rename(HostArea::Home, "Projects/todo.txt", "plan.txt", user)
            .unwrap();
        assert_eq!(
            host.list(HostArea::Home, "Projects", user).unwrap()[0].name,
            "plan.txt"
        );
        host.remove(HostArea::Home, "Projects/plan.txt", user)
            .unwrap();
        host.remove(HostArea::Home, "Projects", user).unwrap();
        assert!(host.list(HostArea::Home, "Projects", user).is_err());

        // Non-empty directories are never removed recursively.
        host.create(HostArea::Home, "", "Full", true, user).unwrap();
        host.create(HostArea::Home, "Full", "keep.txt", false, user)
            .unwrap();
        assert_eq!(
            host.remove(HostArea::Home, "Full", user).unwrap_err(),
            HostError::Invalid
        );
        // Existing targets are never overwritten by create or rename.
        assert_eq!(
            host.create(HostArea::Home, "", "Documents", true, user)
                .unwrap_err(),
            HostError::Invalid
        );
    }
}
