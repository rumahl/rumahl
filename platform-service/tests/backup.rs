use rumahl_core::{AccountStateRepository, PersonalFiles};
use rumahl_persistence_sqlite::{
    HomePersonalFiles, SqliteAccountStateRepository, SqliteAuditLog, SqlitePersonalFiles,
    SqliteShellPreferences,
};
use rumahl_platform_service::user_homes::ensure_user_home;
use rumahl_platform_service::*;
use std::os::unix::fs::PermissionsExt;

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn backup_captures_every_state_database_and_account_home_owner_only() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let home_root = temp.path().join("home");
    std::fs::create_dir(&home_root).unwrap();
    let accounts = state.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "alice",
        password,
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    drop(SqliteShellPreferences::open(state.join("preferences.sqlite")).unwrap());
    drop(SqlitePersonalFiles::open(state.join("files.sqlite")).unwrap());
    drop(SqliteAuditLog::open(state.join("audit.sqlite")).unwrap());

    // The home is the source of truth: its documents and their id index must
    // both be in the backup.
    ensure_user_home(&home_root, "alice").unwrap();
    std::fs::write(home_root.join("alice/Documents/note.txt"), b"remember me").unwrap();
    let user = *SqliteAccountStateRepository::open(&accounts)
        .unwrap()
        .load()
        .unwrap()
        .accounts()
        .accounts()
        .first()
        .unwrap()
        .user_id();
    let index = state.join("files-index.sqlite");
    let files = HomePersonalFiles::open(&home_root, &accounts, &index).unwrap();
    files
        .create(user, "root", "external.txt", Some(b"home".to_vec()))
        .unwrap();

    // A symlink in a home must never pull in files from outside it.
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, b"secret").unwrap();
    std::os::unix::fs::symlink(&outside, home_root.join("alice/leak.txt")).unwrap();

    let destination = temp.path().join("backup");
    backup_state_with_home(&state, &destination, &home_root).unwrap();

    assert_eq!(mode(&destination), 0o700);
    let recovered = SqliteAccountStateRepository::open(destination.join("accounts.sqlite"))
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(recovered.accounts().accounts().len(), 1);
    for name in [
        "preferences.sqlite",
        "files.sqlite",
        "files-index.sqlite",
        "audit.sqlite",
    ] {
        assert!(
            destination.join(name).exists(),
            "{name} missing from backup"
        );
    }

    let home = destination.join("home");
    assert_eq!(mode(&home), 0o700);
    assert_eq!(mode(&home.join("alice")), 0o700);
    let note = home.join("alice/Documents/note.txt");
    assert_eq!(std::fs::read(&note).unwrap(), b"remember me");
    assert_eq!(mode(&note), 0o600);
    assert_eq!(
        std::fs::read(home.join("alice/external.txt")).unwrap(),
        b"home"
    );
    assert!(!home.join("alice/leak.txt").exists());
    assert!(!destination.join("home/outside.txt").exists());
}

#[test]
fn backup_refuses_a_destination_inside_the_home_root() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let home_root = temp.path().join("home");
    std::fs::create_dir(&home_root).unwrap();
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &state.join("accounts.sqlite"),
        "alice",
        "alice",
        password,
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    ensure_user_home(&home_root, "alice").unwrap();

    let destination = home_root.join("alice/backup");
    std::fs::create_dir(&destination).unwrap();
    assert!(backup_state_with_home(&state, &destination, &home_root).is_err());
}
