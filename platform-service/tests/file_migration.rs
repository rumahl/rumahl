//! End-to-end migration of the personal store into the account homes.
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use rumahl_core::{
    AccountStateRepository, GrantAuthority, GrantIssuerPolicy, InMemoryGrantStore, PermissionId,
    PermissionScope, PersonalFiles, PlatformSnapshot, PlatformSnapshotRepository, ResourceKey,
    ResourceKind, ResourceNamespace, ResourceRef, UserId, UserIdentity, UserRole,
};
use rumahl_persistence_sqlite::{
    HomePersonalFiles, SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqlitePersonalFiles, SqliteSnapshotRepository,
};
use rumahl_platform_service::file_migration::migrate_files;
use rumahl_platform_service::user_homes::ensure_user_home;

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
            [9_u8; 32],
        ))
    }

    fn key_by_id(
        &self,
        _id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        <Self as SecretEncryptionKeyProvider>::active_key(self).map(Some)
    }
}

struct Fixture {
    temp: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("home")).unwrap();
        Self { temp }
    }

    fn state(&self) -> std::path::PathBuf {
        self.temp.path().join("state")
    }

    fn home_root(&self) -> std::path::PathBuf {
        self.temp.path().join("home")
    }

    fn accounts(&self) -> std::path::PathBuf {
        self.state().join("accounts.sqlite")
    }

    fn alice(&self) -> UserId {
        std::fs::create_dir_all(self.state()).unwrap();
        let existing = if self.accounts().exists() {
            let state =
                rumahl_persistence_sqlite::SqliteAccountStateRepository::open(self.accounts())
                    .unwrap()
                    .load()
                    .unwrap();
            state
                .accounts()
                .accounts()
                .iter()
                .find(|account| account.username().as_str() == "alice")
                .map(|account| *account.user_id())
        } else {
            None
        };
        if let Some(user) = existing {
            return user;
        }
        let password = rumahl_account_auth::SessionToken::generate()
            .unwrap()
            .encode()
            .to_string();
        rumahl_platform_service::provision(
            &self.accounts(),
            "alice",
            "Alice",
            password,
            rumahl_platform_service::LocalPasswordBlocklist::default(),
        )
        .unwrap();
        ensure_user_home(&self.home_root(), "alice").unwrap();
        let state = rumahl_persistence_sqlite::SqliteAccountStateRepository::open(self.accounts())
            .unwrap()
            .load()
            .unwrap();
        *state
            .accounts()
            .accounts()
            .iter()
            .find(|account| account.username().as_str() == "alice")
            .unwrap()
            .user_id()
    }

    fn seeded_store(&self, encrypted: bool, rows: &[(&str, Option<&[u8]>)]) -> SqlitePersonalFiles {
        let path = self.state().join("files.sqlite");
        let store = if encrypted {
            SqlitePersonalFiles::open_encrypted(&path, Arc::new(KeyProvider)).unwrap()
        } else {
            SqlitePersonalFiles::open(&path).unwrap()
        };
        let alice = self.alice();
        store.create(alice, "root", "Documents", None).unwrap();
        let documents = store
            .list(alice, "root")
            .unwrap()
            .into_iter()
            .find(|entry| entry.name == "Documents")
            .unwrap();
        for (name, content) in rows {
            store
                .create(
                    alice,
                    &documents.id,
                    name,
                    content.map(|bytes| bytes.to_vec()),
                )
                .unwrap();
        }
        store
    }

    fn snapshot(&self, user: UserId) -> (String, String) {
        let state = rumahl_persistence_sqlite::SqliteAccountStateRepository::open(self.accounts())
            .unwrap()
            .load()
            .unwrap();
        let _ = state;
        // One user-issued files grant on a store file and one with an unknown
        // resource that must be dropped.
        let user_identity = UserIdentity::new(user);
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(user, UserRole::User);
        let resource = |key: &str| {
            ResourceRef::new(
                ResourceNamespace::parse("rumahl.files").unwrap(),
                ResourceKind::parse("file").unwrap(),
                ResourceKey::parse(key).unwrap(),
            )
        };
        let known = key_of_store_file(self, user);
        let grant = GrantAuthority::new()
            .issue(
                &policy,
                user_identity.into(),
                user_identity.into(),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![resource(&known)],
            )
            .unwrap();
        let unknown = GrantAuthority::new()
            .issue(
                &policy,
                user_identity.into(),
                user_identity.into(),
                PermissionId::parse("rumahl.files.write").unwrap(),
                PermissionScope::Explicit,
                vec![resource("ffffffffffffffffffffffffffffffff")],
            )
            .unwrap();
        let mut grants = InMemoryGrantStore::new();
        grants.insert(grant.clone());
        grants.insert(unknown);
        let repository =
            SqliteSnapshotRepository::open(self.state().join("platform.sqlite")).unwrap();
        repository
            .store(&PlatformSnapshot::capture(
                &rumahl_core::PlatformState::new(),
                &grants,
            ))
            .unwrap();
        (grant.id().to_string(), known)
    }
}

fn key_of_store_file(fixture: &Fixture, user: UserId) -> String {
    let store = SqlitePersonalFiles::open(fixture.state().join("files.sqlite")).unwrap();
    let documents = store
        .list(user, "root")
        .unwrap()
        .into_iter()
        .find(|entry| entry.directory)
        .unwrap();
    store
        .list(user, &documents.id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "notes.txt")
        .unwrap()
        .id
}

#[test]
fn migrates_documents_into_the_home_and_remaps_grants() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    fixture.seeded_store(false, &[("notes.txt", Some(b"stored"))]);
    let (grant_id, old_id) = fixture.snapshot(alice);

    let report = migrate_files(&fixture.state(), &fixture.home_root(), None).unwrap();

    assert_eq!(report.accounts, 1);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.files, 1);
    assert_eq!(report.directories, 0);
    // The standard Documents folder already exists in a provisioned home.
    assert_eq!(report.existing, 1);
    assert_eq!(report.grants_remapped, 1);
    assert_eq!(report.grants_dropped, 1);
    // The file is a real file in the account's home.
    assert_eq!(
        std::fs::read(fixture.home_root().join("alice/Documents/notes.txt")).unwrap(),
        b"stored"
    );
    // The surviving grant keeps its id and now points at the home file.
    let home = HomePersonalFiles::open(
        fixture.home_root(),
        fixture.accounts(),
        fixture.state().join("files-index.sqlite"),
    )
    .unwrap();
    let documents = home
        .list(alice, "root")
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "Documents")
        .unwrap();
    let notes = home
        .list(alice, &documents.id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "notes.txt")
        .unwrap();
    assert_ne!(notes.id, old_id);
    let snapshot = SqliteSnapshotRepository::open(fixture.state().join("platform.sqlite"))
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    let mut grants = InMemoryGrantStore::new();
    let mut state = rumahl_core::PlatformState::new();
    rumahl_core::PlatformRecovery::new()
        .recover(&snapshot, &mut state, &mut grants)
        .unwrap();
    assert_eq!(grants.len(), 1);
    let grant = grants.grants()[0].clone();
    assert_eq!(grant.id().to_string(), grant_id);
    assert_eq!(grant.resources()[0].key().as_str(), notes.id.as_str());
    // The store is untouched: it stays the rollback.
    let store = SqlitePersonalFiles::open(fixture.state().join("files.sqlite")).unwrap();
    assert!(
        store
            .list(alice, "root")
            .unwrap()
            .iter()
            .any(|entry| entry.name == "Documents")
    );
}

#[test]
fn reruns_do_not_duplicate_or_overwrite() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    fixture.seeded_store(false, &[("notes.txt", Some(b"first"))]);
    fixture.snapshot(alice);
    migrate_files(&fixture.state(), &fixture.home_root(), None).unwrap();

    // A user edit in the home must survive the next migration run.
    std::fs::write(
        fixture.home_root().join("alice/Documents/notes.txt"),
        b"edited",
    )
    .unwrap();
    let report = migrate_files(&fixture.state(), &fixture.home_root(), None).unwrap();

    assert_eq!(report.files, 0);
    assert_eq!(report.directories, 0);
    assert_eq!(report.existing, 2);
    assert_eq!(
        std::fs::read(fixture.home_root().join("alice/Documents/notes.txt")).unwrap(),
        b"edited"
    );
    // The remapped grant already points at a home id and stays untouched; the
    // unresolvable grant was dropped by the first run and stays gone.
    assert_eq!(report.grants_remapped, 1);
    assert_eq!(report.grants_dropped, 0);
}

#[test]
fn sealed_documents_are_re_sealed_in_the_home() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    fixture.seeded_store(true, &[("secret.txt", Some(b"top secret"))]);

    migrate_files(
        &fixture.state(),
        &fixture.home_root(),
        Some(Arc::new(KeyProvider)),
    )
    .unwrap();

    let stored = std::fs::read(fixture.home_root().join("alice/Documents/secret.txt")).unwrap();
    assert!(!stored.windows(10).any(|window| window == b"top secret"));
    let home = HomePersonalFiles::open_encrypted(
        fixture.home_root(),
        fixture.accounts(),
        fixture.state().join("files-index.sqlite"),
        Arc::new(KeyProvider),
    )
    .unwrap();
    let documents = home
        .list(alice, "root")
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "Documents")
        .unwrap();
    let secret = home
        .list(alice, &documents.id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "secret.txt")
        .unwrap();
    assert_eq!(home.read(alice, &secret.id).unwrap().1, b"top secret");
}

#[test]
fn accounts_without_a_home_are_reported_and_their_grants_dropped() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    fixture.seeded_store(false, &[("notes.txt", Some(b"stored"))]);
    fixture.snapshot(alice);
    std::fs::remove_dir_all(fixture.home_root().join("alice")).unwrap();

    let report = migrate_files(&fixture.state(), &fixture.home_root(), None).unwrap();

    // Nothing is invented at a path the account never had; the unresolvable
    // grants are dropped instead of left pointing at the old store.
    assert_eq!(report.accounts, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.files, 0);
    assert_eq!(report.grants_remapped, 0);
    assert_eq!(report.grants_dropped, 2);
    assert!(!fixture.home_root().join("alice").exists());
    // The store is untouched: it stays the rollback.
    let store = SqlitePersonalFiles::open(fixture.state().join("files.sqlite")).unwrap();
    assert!(
        store
            .list(alice, "root")
            .unwrap()
            .iter()
            .any(|entry| entry.name == "Documents")
    );
}
