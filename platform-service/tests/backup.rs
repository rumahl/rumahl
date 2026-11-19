use rumahl_core::AccountStateRepository;
use rumahl_persistence_sqlite::{
    SqliteAccountStateRepository, SqliteAuditLog, SqlitePersonalFiles, SqliteShellPreferences,
};
use rumahl_platform_service::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn backup_captures_every_state_database_owner_only() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    std::fs::create_dir(&state).unwrap();
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

    let destination = temp.path().join("backup");
    backup_state(&state, &destination).unwrap();

    let mode = std::fs::metadata(&destination)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);

    let recovered = SqliteAccountStateRepository::open(destination.join("accounts.sqlite"))
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(recovered.accounts().accounts().len(), 1);
    for name in ["preferences.sqlite", "files.sqlite", "audit.sqlite"] {
        assert!(
            destination.join(name).exists(),
            "{name} missing from backup"
        );
    }
}
