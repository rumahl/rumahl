use rumahl_core::{
    BrowserProfileId, FileError, PersonalFiles, PreferenceScope, ShellPreferencesError, UserId,
    WorkspaceRepository,
};
use rumahl_persistence_sqlite::{SqlitePersonalFiles, SqliteShellPreferences};
#[test]
fn documents_are_owned_durable_bounded_and_moves_cannot_create_cycles() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("files.sqlite");
    let repo = SqlitePersonalFiles::open(&path).unwrap();
    let a = UserId::new();
    let b = UserId::new();
    repo.create(a, "root", "Documents", None).unwrap();
    let folder = repo.list(a, "root").unwrap().remove(0);
    repo.create(a, &folder.id, "hello.txt", Some(b"hello".to_vec()))
        .unwrap();
    let file = repo.list(a, &folder.id).unwrap().remove(0);
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
        .find(|f| f.directory)
        .unwrap();
    assert_eq!(
        repo.relocate(a, &folder.id, &child.id, "Documents"),
        Err(FileError::Invalid)
    );
    repo.relocate(a, &file.id, "root", "renamed.txt").unwrap();
    drop(repo);
    let repo = SqlitePersonalFiles::open(path).unwrap();
    assert_eq!(
        repo.read(a, &file.id).unwrap(),
        ("renamed.txt".into(), b"hello".to_vec())
    );
    assert_eq!(
        repo.create(a, "root", "renamed.txt", Some(vec![])),
        Err(FileError::Conflict)
    );
    repo.delete(a, &child.id).unwrap();
    repo.delete(a, &folder.id).unwrap();
    repo.delete(a, &file.id).unwrap();
    assert!(repo.list(a, "root").unwrap().is_empty());
}
#[test]
fn workspaces_inherit_isolate_and_compare_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("prefs.sqlite");
    let repo = SqliteShellPreferences::open(&path).unwrap();
    let a = UserId::new();
    let b = UserId::new();
    let d = BrowserProfileId::parse("00000000-0000-4000-8000-000000000001").unwrap();
    let other = BrowserProfileId::parse("00000000-0000-4000-8000-000000000002").unwrap();
    repo.save_workspace(a, d, 0, PreferenceScope::User, Some("account".into()))
        .unwrap();
    repo.save_workspace(a, d, 1, PreferenceScope::Device, Some("device".into()))
        .unwrap();
    assert_eq!(
        repo.load_workspace(a, other).unwrap().user.as_deref(),
        Some("account")
    );
    assert!(repo.load_workspace(a, other).unwrap().device.is_none());
    assert!(repo.load_workspace(b, d).unwrap().user.is_none());
    assert!(matches!(
        repo.save_workspace(a, d, 0, PreferenceScope::User, None),
        Err(ShellPreferencesError::Conflict)
    ));
    drop(repo);
    let repo = SqliteShellPreferences::open(path).unwrap();
    assert_eq!(
        repo.load_workspace(a, d).unwrap().device.as_deref(),
        Some("device")
    );
    repo.save_workspace(a, d, 2, PreferenceScope::Device, None)
        .unwrap();
    assert!(repo.load_workspace(a, d).unwrap().device.is_none());
}
