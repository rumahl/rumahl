//! Real host roots the file explorer may browse, backed by the OS layout.
use rumahl_platform_buildroot::FilesystemLayout;
use rumahl_platform_web::{HostArea, HostEntry, HostError, HostFiles};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_ENTRIES: usize = 2000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Reads `/apps` and the protected `/rumahl` tree. The roots come from
/// `RUMAHL_APPS_ROOT` / `RUMAHL_SYSTEM_ROOT`, falling back to the layout.
pub struct LocalHostFiles {
    apps_root: PathBuf,
    system_root: PathBuf,
}

impl LocalHostFiles {
    pub fn new(apps_root: PathBuf, system_root: PathBuf) -> Self {
        Self {
            apps_root,
            system_root,
        }
    }

    pub fn from_environment() -> Self {
        let layout = FilesystemLayout::system();
        let apps_root = std::env::var_os("RUMAHL_APPS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| layout.apps_root().to_path_buf());
        let system_root = std::env::var_os("RUMAHL_SYSTEM_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| layout.system_root().to_path_buf());
        Self::new(apps_root, system_root)
    }

    fn root(&self, area: HostArea) -> &Path {
        match area {
            HostArea::Apps => &self.apps_root,
            HostArea::System => &self.system_root,
        }
    }

    /// Resolves a relative, client-supplied path inside the area root. Rejects
    /// traversal and verifies the canonical target stays under the real root.
    fn resolve(&self, area: HostArea, relative: &str) -> Result<PathBuf, HostError> {
        if relative.contains('\0') || relative.starts_with('/') {
            return Err(HostError::Invalid);
        }
        let root = self
            .root(area)
            .canonicalize()
            .map_err(|_| HostError::Missing)?;
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
}

impl HostFiles for LocalHostFiles {
    fn list(&self, area: HostArea, path: &str) -> Result<Vec<HostEntry>, HostError> {
        let directory = self.resolve(area, path)?;
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

    fn read(&self, area: HostArea, path: &str) -> Result<(String, Vec<u8>), HostError> {
        let file = self.resolve(area, path)?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> (tempfile::TempDir, LocalHostFiles) {
        let temp = tempfile::tempdir().unwrap();
        let apps = temp.path().join("apps");
        let system = temp.path().join("system");
        std::fs::create_dir_all(apps.join("com.example.notes/data")).unwrap();
        std::fs::write(apps.join("com.example.notes/data/notes.txt"), b"hello").unwrap();
        std::fs::create_dir_all(&system).unwrap();
        (temp, LocalHostFiles::new(apps, system))
    }

    #[test]
    fn lists_directories_and_files() {
        let (_temp, host) = roots();
        let entries = host.list(HostArea::Apps, "com.example.notes/data").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "notes.txt");
        assert!(!entries[0].directory);
        assert_eq!(entries[0].size, 5);
    }

    #[test]
    fn reads_files_and_rejects_traversal_and_symlinks() {
        let (temp, host) = roots();
        let (name, data) = host
            .read(HostArea::Apps, "com.example.notes/data/notes.txt")
            .unwrap();
        assert_eq!(name, "notes.txt");
        assert_eq!(data, b"hello");

        assert_eq!(
            host.list(HostArea::Apps, "../system").unwrap_err(),
            HostError::Invalid
        );
        assert_eq!(
            host.read(HostArea::Apps, "/etc/passwd").unwrap_err(),
            HostError::Invalid
        );

        std::os::unix::fs::symlink(temp.path().join("system"), host.apps_root.join("escape"))
            .unwrap();
        assert!(host.list(HostArea::Apps, "escape").is_err());
    }
}
