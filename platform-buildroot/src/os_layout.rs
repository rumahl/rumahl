//! The rumahl OS filesystem layout.
//!
//! rumahl addresses files with real host paths and defines a small, protected
//! top-level layout:
//!
//! * `/rumahl` — the protected system tree (mode `0700` on the read-only system
//!   disk). It holds the system services and the admin home `/rumahl/root`,
//!   which replaces the classic `/root`.
//! * `/apps` — installed applications, one directory per app id.
//! * `/home` — user homes; every account gets the standard subdirectories below.
//!
//! `/root` is intentionally absent. The roots are fixed for the first milestone;
//! Buildroot will make them configurable later.

use std::path::{Path, PathBuf};

/// Protected system tree that replaces `/root`.
pub const SYSTEM_ROOT: &str = "/rumahl";

/// The admin home that replaces `/root`.
pub const ADMIN_HOME: &str = "/rumahl/root";

/// Installed applications, one directory per app id.
pub const APPS_ROOT: &str = "/apps";

/// User homes.
pub const HOME_ROOT: &str = "/home";

/// Private system state: databases and secrets, never mounted into the
/// user- or app-visible path namespace.
pub const STATE_ROOT: &str = "/var/lib/rumahl";

/// Standard subdirectories created for every account.
pub const USER_DIRECTORIES: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Music",
    "Pictures",
    "Public",
    "Templates",
    "Videos",
];

/// Configurable rumahl filesystem roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemLayout {
    system_root: PathBuf,
    apps_root: PathBuf,
    home_root: PathBuf,
    state_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    RootMustBeAbsolute,
}

impl FilesystemLayout {
    /// The fixed first-milestone layout.
    pub fn system() -> Self {
        Self::new(SYSTEM_ROOT, APPS_ROOT, HOME_ROOT, STATE_ROOT)
            .expect("the default rumahl layout roots are absolute")
    }

    pub fn new(
        system_root: impl Into<PathBuf>,
        apps_root: impl Into<PathBuf>,
        home_root: impl Into<PathBuf>,
        state_root: impl Into<PathBuf>,
    ) -> Result<Self, LayoutError> {
        let system_root = system_root.into();
        let apps_root = apps_root.into();
        let home_root = home_root.into();
        let state_root = state_root.into();
        for root in [&system_root, &apps_root, &home_root, &state_root] {
            if !root.is_absolute() {
                return Err(LayoutError::RootMustBeAbsolute);
            }
        }
        Ok(Self {
            system_root,
            apps_root,
            home_root,
            state_root,
        })
    }

    pub fn system_root(&self) -> &Path {
        &self.system_root
    }

    /// The admin home (`<system_root>/root`) that replaces `/root`.
    pub fn admin_home(&self) -> PathBuf {
        self.system_root.join("root")
    }

    pub fn apps_root(&self) -> &Path {
        &self.apps_root
    }

    pub fn app_dir(&self, app_id: &str) -> PathBuf {
        self.apps_root.join(app_id)
    }

    pub fn home_root(&self) -> &Path {
        &self.home_root
    }

    pub fn user_home(&self, user: &str) -> PathBuf {
        self.home_root.join(user)
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootMustBeAbsolute => write!(f, "rumahl layout roots must be absolute paths"),
        }
    }
}

impl std::error::Error for LayoutError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_matches_the_documented_paths() {
        let layout = FilesystemLayout::system();
        assert_eq!(layout.system_root(), Path::new("/rumahl"));
        assert_eq!(layout.admin_home(), Path::new("/rumahl/root"));
        assert_eq!(layout.apps_root(), Path::new("/apps"));
        assert_eq!(layout.home_root(), Path::new("/home"));
        assert_eq!(layout.state_root(), Path::new("/var/lib/rumahl"));
    }

    #[test]
    fn derives_app_and_user_directories() {
        let layout = FilesystemLayout::system();
        assert_eq!(
            layout.app_dir("com.example.notes"),
            Path::new("/apps/com.example.notes")
        );
        assert_eq!(layout.user_home("alice"), Path::new("/home/alice"));
    }

    #[test]
    fn lists_the_standard_user_directories() {
        for directory in [
            "Desktop",
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "Videos",
        ] {
            assert!(USER_DIRECTORIES.contains(&directory), "missing {directory}");
        }
    }

    #[test]
    fn rejects_relative_roots() {
        assert_eq!(
            FilesystemLayout::new("rumahl", "/apps", "/home", "/var/lib/rumahl").unwrap_err(),
            LayoutError::RootMustBeAbsolute
        );
    }
}
