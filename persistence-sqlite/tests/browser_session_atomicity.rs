use rumahl_account_auth::{
    LocalAccountAdministrationService, PasswordAuthenticationService, PasswordBlocklist,
};
use rumahl_core::{AccountStateRepository, UnixTimestamp};
use rumahl_persistence_sqlite::{
    BrowserSessionError, SqliteAccountStateRepository, SqliteBrowserSessionRepository,
    SqliteLocalAccountAdministrationRepository, SqlitePasswordCredentialRepository,
};
struct Blocklist;
impl PasswordBlocklist for Blocklist {
    fn contains(&self, _: &str) -> bool {
        false
    }
}

#[test]
fn password_change_invalidates_in_flight_login_proof_and_failed_commit_leaves_no_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.sqlite");
    let accounts = SqliteAccountStateRepository::open(&path).unwrap();
    let mut state = accounts.load().unwrap();
    let admin = LocalAccountAdministrationService::new(
        SqliteLocalAccountAdministrationRepository::open(&path).unwrap(),
        Blocklist,
    );
    let user = admin
        .provision_password_account(
            &mut state,
            "alice",
            "Alice",
            "original long test password".into(),
            UnixTimestamp::from_seconds(10),
        )
        .unwrap();
    let auth = PasswordAuthenticationService::new(
        SqlitePasswordCredentialRepository::open(&path).unwrap(),
        Blocklist,
    )
    .unwrap();
    let proof = auth
        .authenticate(
            &state,
            "alice",
            "original long test password".into(),
            UnixTimestamp::from_seconds(11),
        )
        .unwrap();
    auth.set_password(
        &state,
        &user,
        "replacement long test password".into(),
        UnixTimestamp::from_seconds(12),
    )
    .unwrap();
    let sessions = SqliteBrowserSessionRepository::open(&path).unwrap();
    assert!(matches!(
        sessions.create(proof, UnixTimestamp::from_seconds(100)),
        Err(BrowserSessionError::InvalidProof)
    ));
    assert!(accounts.load().unwrap().sessions().is_empty());

    let proof = auth
        .authenticate(
            &state,
            "alice",
            "replacement long test password".into(),
            UnixTimestamp::from_seconds(13),
        )
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_credential BEFORE INSERT ON session_credential BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert!(matches!(
        sessions.create(proof, UnixTimestamp::from_seconds(100)),
        Err(BrowserSessionError::Storage)
    ));
    assert!(accounts.load().unwrap().sessions().is_empty());
    let credentials: i64 = connection
        .query_row("SELECT count(*) FROM session_credential", [], |r| r.get(0))
        .unwrap();
    assert_eq!(credentials, 0);
}
