//! Contract tests for the home-backed personal-files store.
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rumahl_account_auth::LocalAccountAdministrationService;
use rumahl_core::{AccountStateRepository, FileError, PersonalFiles, UnixTimestamp, UserId};
use rumahl_persistence_sqlite::{
    HomePersonalFiles, SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqliteAccountStateRepository, SqliteLocalAccountAdministrationRepository,
};

#[derive(Debug)]
struct TestError;

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("key error")
    }
}
impl Error for TestError {}

struct KeyProvider;

impl SecretEncryptionKeyProvider for KeyProvider {
    type Error = TestError;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
        Ok(SecretEncryptionKey::new(
            SecretEncryptionKeyId::parse("key-1").unwrap(),
            [7_u8; 32],
        ))
    }

    fn key_by_id(
        &self,
        _id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        <Self as SecretEncryptionKeyProvider>::active_key(self).map(Some)
    }
}

/// A disposable deployment: home root, accounts database and id index.
struct Fixture {
    temp: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("home")).unwrap();
        Self { temp }
    }

    fn home_root(&self) -> PathBuf {
        self.temp.path().join("home")
    }

    fn accounts(&self) -> PathBuf {
        self.temp.path().join("accounts.sqlite")
    }

    fn index(&self) -> PathBuf {
        self.temp.path().join("index.sqlite")
    }

    fn user(&self, username: &str) -> UserId {
        let accounts = SqliteAccountStateRepository::open(self.accounts()).unwrap();
        let mut state = accounts.load().unwrap();
        struct Blocklist;
        impl rumahl_account_auth::PasswordBlocklist for Blocklist {
            fn contains(&self, _: &str) -> bool {
                false
            }
        }
        let admin = LocalAccountAdministrationService::new(
            SqliteLocalAccountAdministrationRepository::open(self.accounts()).unwrap(),
            Blocklist,
        );
        let user = admin
            .provision_password_account(
                &mut state,
                username,
                username,
                format!("test password for {username}"),
                UnixTimestamp::from_seconds(10),
            )
            .unwrap_or_else(|_| panic!("provision {username}"));
        std::fs::create_dir_all(self.home_root().join(username)).unwrap();
        user
    }

    fn store(&self) -> HomePersonalFiles {
        HomePersonalFiles::open(self.home_root(), self.accounts(), self.index()).unwrap()
    }

    fn sealed(&self) -> HomePersonalFiles {
        HomePersonalFiles::open_encrypted(
            self.home_root(),
            self.accounts(),
            self.index(),
            Arc::new(KeyProvider),
        )
        .unwrap()
    }
}

fn home_file(home: &Path, relative: &str) -> PathBuf {
    let mut path = home.to_path_buf();
    for segment in relative.split('/') {
        if !segment.is_empty() {
            path.push(segment);
        }
    }
    path
}

#[test]
fn documents_are_owned_durable_bounded_and_moves_cannot_create_cycles() {
    let fixture = Fixture::new();
    let a = fixture.user("alice");
    let b = fixture.user("bob");
    let repo = fixture.store();
    repo.create(a, "root", "Documents", None).unwrap();
    let folder = repo.list(a, "root").unwrap().remove(0);
    repo.create(a, &folder.id, "hello.txt", Some(b"hello".to_vec()))
        .unwrap();
    let file = repo.list(a, &folder.id).unwrap().remove(0);

    // Names are real files in the account's home.
    assert_eq!(
        std::fs::read(home_file(&fixture.home_root(), "alice/Documents/hello.txt")).unwrap(),
        b"hello"
    );
    // Other accounts see nothing and cannot address these ids.
    assert!(repo.list(b, "root").unwrap().is_empty());
    assert_eq!(repo.read(b, &file.id), Err(FileError::Missing));
    assert_eq!(
        repo.create(b, &folder.id, "bad", None),
        Err(FileError::Missing)
    );
    assert_eq!(
        repo.relocate(b, &file.id, "root", "stolen"),
        Err(FileError::Missing)
    );
    assert_eq!(repo.delete(b, &file.id), Err(FileError::Missing));

    // The store contract holds unchanged.
    assert_eq!(
        repo.create(a, "root", "../escape", None),
        Err(FileError::Invalid)
    );
    assert_eq!(
        repo.create(a, "root", "large", Some(vec![0; 16 * 1024 * 1024 + 1])),
        Err(FileError::Limit)
    );
    assert_eq!(repo.delete(a, &folder.id), Err(FileError::Conflict));
    repo.create(a, &folder.id, "Child", None).unwrap();
    let child = repo
        .list(a, &folder.id)
        .unwrap()
        .into_iter()
        .find(|file| file.directory)
        .unwrap();
    assert_eq!(
        repo.relocate(a, &folder.id, &child.id, "Documents"),
        Err(FileError::Invalid)
    );

    // A rename keeps the id, so grants survive it.
    repo.relocate(a, &file.id, "root", "renamed.txt").unwrap();
    assert_eq!(
        repo.read(a, &file.id).unwrap(),
        ("renamed.txt".into(), b"hello".to_vec())
    );
    assert_eq!(
        std::fs::read(home_file(&fixture.home_root(), "alice/renamed.txt")).unwrap(),
        b"hello"
    );

    drop(repo);
    let repo = fixture.store();
    repo.write(a, &file.id, b"edited text".to_vec()).unwrap();
    assert_eq!(
        repo.read(a, &file.id).unwrap(),
        ("renamed.txt".into(), b"edited text".to_vec())
    );
    assert_eq!(
        repo.write(b, &file.id, b"stolen".to_vec()),
        Err(FileError::Missing)
    );
    assert_eq!(
        repo.write(a, &folder.id, b"not a file".to_vec()),
        Err(FileError::Missing)
    );
    assert_eq!(
        repo.write(a, &file.id, vec![0; 16 * 1024 * 1024 + 1]),
        Err(FileError::Limit)
    );
    assert_eq!(
        repo.create(a, "root", "renamed.txt", Some(vec![])),
        Err(FileError::Conflict)
    );

    repo.delete(a, &child.id).unwrap();
    repo.delete(a, &folder.id).unwrap();
    repo.delete(a, &file.id).unwrap();
    assert!(repo.list(a, "root").unwrap().is_empty());
    assert!(!home_file(&fixture.home_root(), "alice/renamed.txt").exists());
}

#[test]
fn lists_paginated_and_filtered_pages() {
    let fixture = Fixture::new();
    let user = fixture.user("alice");
    let repo = fixture.store();
    for name in ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"] {
        repo.create(user, "root", name, Some(Vec::new())).unwrap();
    }

    let (first, total) = repo.list_page(user, "root", 0, 2, None).unwrap();
    assert_eq!(total, 5);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].name, "Alpha");
    assert_eq!(first[1].name, "Beta");

    let (second, total) = repo.list_page(user, "root", 2, 2, None).unwrap();
    assert_eq!(total, 5);
    assert_eq!(second[0].name, "Delta");

    let (filtered, filtered_total) = repo.list_page(user, "root", 0, 10, Some("A")).unwrap();
    assert_eq!(filtered_total, 4);
    assert!(
        filtered
            .iter()
            .all(|file| file.name.to_lowercase().contains('a'))
    );
}

#[test]
fn external_files_appear_with_a_stable_id_and_symlinks_stay_hidden() {
    let fixture = Fixture::new();
    let user = fixture.user("alice");
    let home = fixture.home_root().join("alice");
    std::fs::write(home.join("external.txt"), b"from another tool").unwrap();
    // A file outside the home, reachable only through a symlink.
    let outside = fixture.temp.path().join("outside.txt");
    std::fs::write(&outside, b"secret").unwrap();
    std::os::unix::fs::symlink(&outside, home.join("link.txt")).unwrap();

    let repo = fixture.store();
    let entries = repo.list(user, "root").unwrap();
    assert_eq!(entries.len(), 1, "symlinks are never listed");
    let external = &entries[0];
    assert_eq!(external.name, "external.txt");
    assert_eq!(
        repo.read(user, &external.id).unwrap().1,
        b"from another tool"
    );
    // The id is derived from account and path, so a restart keeps it.
    drop(repo);
    let repo = fixture.store();
    assert_eq!(repo.list(user, "root").unwrap()[0].id, external.id);
    // The symlink was never touched, listed or followed.
    assert_eq!(std::fs::read(&outside).unwrap(), b"secret");
}

#[test]
fn sealed_content_is_not_readable_in_the_home() {
    let fixture = Fixture::new();
    let user = fixture.user("alice");
    let repo = fixture.sealed();
    repo.create(user, "root", "note.txt", Some(b"top secret".to_vec()))
        .unwrap();
    let file = repo.list(user, "root").unwrap().remove(0);
    let stored = std::fs::read(home_file(&fixture.home_root(), "alice/note.txt")).unwrap();
    assert!(!stored.windows(10).any(|window| window == b"top secret"));
    assert_eq!(repo.read(user, &file.id).unwrap().1, b"top secret");

    // Plain files written by other tools stay readable next to sealed ones.
    std::fs::write(home_file(&fixture.home_root(), "alice/plain.txt"), b"plain").unwrap();
    let plain = repo
        .list(user, "root")
        .unwrap()
        .into_iter()
        .find(|file| file.name == "plain.txt")
        .unwrap();
    assert_eq!(repo.read(user, &plain.id).unwrap().1, b"plain");

    // Without the key provider a sealed file reports unavailable, never
    // garbage or plaintext.
    let unsealed = fixture.store();
    assert_eq!(unsealed.read(user, &file.id), Err(FileError::Unavailable));
}
