//! Per-account home directories.
//!
//! Every account owns `/home/<username>` with the standard user directories
//! from the OS layout. The platform service creates them when an account is
//! provisioned and heals missing homes at startup, so the writable home area
//! in the file explorer always has somewhere to point. Creation is
//! best-effort: an account whose home cannot be created stays usable (the
//! personal store is independent), and the home area simply reports absent
//! rather than exposing a wrong directory.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use rumahl_core::AccountStateRepository;
use rumahl_persistence_sqlite::SqliteAccountStateRepository;
use rumahl_platform_buildroot::{FilesystemLayout, USER_DIRECTORIES};

const HOME_MODE: u32 = 0o700;

/// The configured home root: `RUMAHL_HOME_ROOT` or the OS layout.
pub fn home_root() -> PathBuf {
    std::env::var_os("RUMAHL_HOME_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| FilesystemLayout::system().home_root().to_path_buf())
}

/// A directory name we are willing to create on the host. Account usernames
/// are already validated by the auth layer; this is defence in depth for the
/// one place a username becomes a host path.
fn usable_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 64
        && username != "."
        && username != ".."
        && !username.contains(['/', '\\', '\0'])
        && !username.chars().any(char::is_control)
}

/// Creates or completes one account's home directory. Idempotent: existing
/// directories keep their contents, missing standard directories are added.
pub fn ensure_user_home(home_root: &Path, username: &str) -> io::Result<PathBuf> {
    if !usable_username(username) {
        return Err(io::Error::other(
            "unusable account username for a home directory",
        ));
    }
    let home = home_root.join(username);
    create_directory(&home)?;
    for directory in USER_DIRECTORIES {
        create_directory(&home.join(directory))?;
    }
    Ok(home)
}

fn create_directory(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => Err(io::Error::other(
            "home directory path is not a plain directory",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir_all(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(HOME_MODE))
        }
        Err(error) => Err(error),
    }
}

/// Ensures a home for every account of the deployment. Returns how many homes
/// existed or were created; individual failures are skipped, never fatal.
pub fn ensure_account_homes(
    accounts: &Path,
    home_root: &Path,
) -> Result<usize, crate::ServiceError> {
    let state = SqliteAccountStateRepository::open(accounts)?.load()?;
    let mut ensured = 0;
    for account in state.accounts().accounts() {
        if ensure_user_home(home_root, account.username().as_str()).is_ok() {
            ensured += 1;
        }
    }
    Ok(ensured)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::LocalPasswordBlocklist;

    fn account(temp: &Path, username: &str) {
        crate::provision(
            &temp.join("accounts.sqlite"),
            username,
            "Test",
            rumahl_account_auth::SessionToken::generate()
                .unwrap()
                .encode()
                .to_string(),
            LocalPasswordBlocklist::default(),
        )
        .unwrap();
    }

    #[test]
    fn creates_the_home_with_the_standard_directories() {
        let temp = tempfile::tempdir().unwrap();
        let home = ensure_user_home(temp.path(), "alice").unwrap();

        assert_eq!(home, temp.path().join("alice"));
        for directory in USER_DIRECTORIES {
            assert!(home.join(directory).is_dir(), "missing {directory}");
        }
        assert_eq!(
            std::fs::metadata(&home).unwrap().permissions().mode() & 0o777,
            HOME_MODE
        );
        // Idempotent: a second pass keeps existing content.
        std::fs::write(home.join("Documents/notes.txt"), b"keep").unwrap();
        ensure_user_home(temp.path(), "alice").unwrap();
        assert_eq!(
            std::fs::read(home.join("Documents/notes.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn refuses_unusable_names_and_symlinked_homes() {
        let temp = tempfile::tempdir().unwrap();
        assert!(ensure_user_home(temp.path(), "../escape").is_err());
        assert!(ensure_user_home(temp.path(), "a/b").is_err());
        assert!(ensure_user_home(temp.path(), "").is_err());

        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, temp.path().join("alice")).unwrap();
        assert!(ensure_user_home(temp.path(), "alice").is_err());
    }

    #[test]
    fn heals_every_existing_account() {
        let temp = tempfile::tempdir().unwrap();
        account(temp.path(), "alice");
        account(temp.path(), "bob");
        std::fs::create_dir_all(temp.path().join("home/alice")).unwrap();

        let ensured = ensure_account_homes(
            &temp.path().join("accounts.sqlite"),
            &temp.path().join("home"),
        )
        .unwrap();

        assert_eq!(ensured, 2);
        assert!(temp.path().join("home/alice/Documents").is_dir());
        assert!(temp.path().join("home/bob/Documents").is_dir());
    }
}
