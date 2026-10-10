use rumahl_core::{
    AccountStateRepository, AuditAction, AuditLog, AuditOutcome, BrowserProfileId, OsMode,
    OsModeRepository, PreferenceScope,
};
use rumahl_persistence_sqlite::{
    SqliteAccountStateRepository, SqliteAuditLog, SqliteOsModeRepository,
};
use rumahl_platform_service::*;
use rumahl_platform_web::BrowserSessions;
use std::sync::Arc;

#[test]
fn authentication_and_mode_changes_are_audited_without_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let accounts = root.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "alice",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();

    let state = SqliteAccountStateRepository::open(&accounts)
        .unwrap()
        .load()
        .unwrap();
    let user = *state.accounts().accounts()[0].user_id();

    let audit = Arc::new(SqliteAuditLog::open(root.join("audit.sqlite")).unwrap());
    let sessions = LocalBrowserSessions::open(&accounts)
        .unwrap()
        .with_audit(audit.clone());

    assert!(sessions.login("alice", password.clone()).is_ok());
    assert!(
        sessions
            .login("alice", "wrong-password".to_owned())
            .is_err()
    );
    assert!(sessions.reauthenticate(user, password).is_ok());
    assert!(
        sessions
            .reauthenticate(user, "wrong-password".to_owned())
            .is_err()
    );

    let mode_repository = AuditedOsModeRepository::new(
        Arc::new(SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap()),
        audit.clone(),
    );
    let device = BrowserProfileId::parse("00000000-0000-4000-8000-000000000001").unwrap();
    mode_repository
        .save(
            user,
            device,
            0,
            PreferenceScope::User,
            Some(OsMode::Developer),
        )
        .unwrap();

    let events = audit.recent(user, 100).unwrap();
    assert!(events.iter().any(|event| {
        event.action == AuditAction::OsModeChanged
            && event.outcome == AuditOutcome::Success
            && event.target.as_deref() == Some("developer")
    }));
    assert!(events.iter().any(|event| {
        event.action == AuditAction::Reauthenticate && event.outcome == AuditOutcome::Success
    }));
    assert!(events.iter().any(|event| {
        event.action == AuditAction::ReauthenticateFailed && event.outcome == AuditOutcome::Denied
    }));
    assert!(events.iter().any(|event| {
        event.action == AuditAction::SignIn && event.outcome == AuditOutcome::Success
    }));
    // Failed sign-ins have no trustworthy user and stay out of the user's view.
    assert!(
        !events
            .iter()
            .any(|event| event.action == AuditAction::SignInFailed)
    );
}
