//! Router-level consent and app file access flow over the real service stack.
use rumahl_core::*;
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqlitePersonalFiles};
use rumahl_platform_service::apps::PersistentApps;
use rumahl_platform_service::*;
use rumahl_platform_web::{
    AppAccess, BrowserSessions, GatewayConfig, GatewayState, InMemoryShellEvents, ShellIdentity,
    router,
};
use std::sync::Arc;
use tower::ServiceExt;

const ORIGIN: &str = "https://rumahl.test";

/// Seeds one installed web app that requests the files permissions, with a
/// launch grant for the given user, and returns its installation id.
fn seed_files_app(root: &std::path::Path, username: &str) -> InstallationId {
    let accounts = SqliteAccountStateRepository::open(root.join("accounts.sqlite"))
        .unwrap()
        .load()
        .unwrap();
    let user = accounts
        .accounts()
        .accounts()
        .iter()
        .find(|user| user.username().as_str() == username)
        .unwrap();
    let user_id = *user.user_id();
    let repository =
        rumahl_persistence_sqlite::SqliteSnapshotRepository::open(root.join("platform.sqlite"))
            .unwrap();
    let mut runtime = RuntimeDescriptor::web();
    runtime
        .add_entrypoint(RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        ))
        .unwrap();
    let mut manifest = AppManifest::new(
        AppId::parse("com.rumahl.files-app").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Files test app",
        runtime,
    )
    .unwrap();
    manifest
        .add_permission_request(PermissionRequest::new(
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::UserSelected,
            false,
            Some("Open the files you pick".to_owned()),
        ))
        .unwrap();
    manifest
        .add_permission_request(PermissionRequest::new(
            PermissionId::parse("rumahl.files.write").unwrap(),
            PermissionScope::UserSelected,
            false,
            Some("Save changes back to the files you pick".to_owned()),
        ))
        .unwrap();

    let mut state = PlatformState::new();
    let installed = AppLifecycle::new().install(manifest, &mut state).unwrap();
    let installation = *installed.installation_id();

    let mut policy = GrantIssuerPolicy::new();
    policy.set_user_role(user_id, UserRole::Owner);
    let subject = UserIdentity::new(user_id);
    let launch = GrantAuthority::new()
        .issue(
            &policy,
            subject.into(),
            subject.into(),
            PermissionId::parse("rumahl.apps.launch").unwrap(),
            PermissionScope::Explicit,
            vec![ResourceRef::new(
                ResourceNamespace::parse("rumahl.apps").unwrap(),
                ResourceKind::parse("installation").unwrap(),
                ResourceKey::parse(installation.to_string()).unwrap(),
            )],
        )
        .unwrap();
    let mut grants = InMemoryGrantStore::new();
    grants.insert(launch);

    let assets = root
        .join("app-assets")
        .join(installation.to_string())
        .join("frontend");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("index.html"), "<!doctype html><p>ready</p>").unwrap();

    repository
        .store(&PlatformSnapshot::capture(&state, &grants))
        .unwrap();
    installation
}

/// One complete gateway over the seeded state, freshly wired like a restart.
fn gateway(root: &std::path::Path) -> axum::Router {
    let accounts = root.join("accounts.sqlite");
    let platform = root.join("platform.sqlite");
    let files: Arc<dyn PersonalFiles> =
        Arc::new(SqlitePersonalFiles::open(root.join("files.sqlite")).unwrap());
    let provider = PersistentApps::open(&accounts, &platform, root.join("app-assets"))
        .unwrap()
        .with_account_roles(&accounts)
        .unwrap()
        .with_personal_files(files.clone());
    let config = GatewayConfig::new(ORIGIN, root.join("ssr.sock")).unwrap();
    let sessions = Arc::new(LocalBrowserSessions::open(&accounts).unwrap());
    router(GatewayState {
        host_files: None,
        weather_warnings: None,
        config: GatewayConfig::new(ORIGIN, root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &platform, "test", "en").unwrap(),
        browser_sessions: Some(sessions),
        admin: None,
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        audit: None,
        state_dir: None,
        platform_config: None,
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: Some(Arc::new(
            AppAccess::new(
                Arc::new(provider),
                &config.public_origin,
                &config.allowed_origins,
                &["apps.rumahl.test".to_owned()],
                None,
            )
            .unwrap(),
        )),
        oidc: None,
        files: Some(files),
        workspace: None,
        preferences: None,
        os_mode: None,
        policies: Some(Arc::new(
            rumahl_platform_service::policy_engine::LocalPolicyEngine::open(&accounts).unwrap(),
        )),
    })
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    use http_body_util::BodyExt;
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn grants_request(token: &str, installation: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/grants?installationId={installation}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .body(axum::body::Body::empty())
        .unwrap()
}

fn file_read_request(
    token: &str,
    installation: &str,
    key: &str,
) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/files/content?installationId={installation}&key={key}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .body(axum::body::Body::empty())
        .unwrap()
}

fn file_write_request(
    token: &str,
    installation: &str,
    key: &str,
    bytes: &'static [u8],
) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("PUT")
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/files/content?installationId={installation}&key={key}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .header("origin", ORIGIN)
        .header("content-type", "application/octet-stream")
        .body(axum::body::Body::from(bytes.to_vec()))
        .unwrap()
}

fn issue_request(
    token: &str,
    installation: &str,
    body: serde_json::Value,
) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("PUT")
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/grants?installationId={installation}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

fn revoke_request(
    token: &str,
    installation: &str,
    grant: &str,
) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("DELETE")
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/grants/{grant}?installationId={installation}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .header("origin", ORIGIN)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
async fn consent_and_file_access_follow_the_grants() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let accounts = root.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "Alice",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let sessions = LocalBrowserSessions::open(&accounts).unwrap();
    let alice = sessions.login("alice", password.clone()).unwrap();
    let installation = seed_files_app(root, "alice");

    // Two personal files for Alice.
    let files = SqlitePersonalFiles::open(root.join("files.sqlite")).unwrap();
    let user_id = {
        let state = SqliteAccountStateRepository::open(&accounts)
            .unwrap()
            .load()
            .unwrap();
        *state
            .accounts()
            .accounts()
            .iter()
            .find(|user| user.username().as_str() == "alice")
            .unwrap()
            .user_id()
    };
    files
        .create(
            user_id,
            "root",
            "notes.txt",
            Some(b"original notes".to_vec()),
        )
        .unwrap();
    files
        .create(user_id, "root", "diary.txt", Some(b"secret diary".to_vec()))
        .unwrap();
    let entries = files.list(user_id, "root").unwrap();
    let key_of = |name: &str| {
        entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap()
            .id
            .clone()
    };
    let notes = key_of("notes.txt");
    let diary = key_of("diary.txt");

    let app = gateway(root);

    // Unauthenticated consent reads are refused.
    let response = app
        .clone()
        .oneshot(grants_request("nope", &installation.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

    // The consent view lists the manifest requests and starts empty.
    let response = app
        .clone()
        .oneshot(grants_request(&alice, &installation.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = json(response).await;
    assert_eq!(value["requests"][0]["permission"], "rumahl.files.read");
    assert_eq!(value["grants"].as_array().unwrap().len(), 0);

    // Without any grant, access reports that consent is missing — for a
    // permission the app actually requested.
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &notes))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "approval-required");

    // Consent for this session only, naming exactly one file.
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            serde_json::json!({
                "permission": "rumahl.files.read",
                "resources": [notes],
                "validity": "session",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let grant = json(response).await;
    assert_eq!(grant["validity"], "session");
    assert_eq!(grant["scope"], "explicit");
    let session_grant_id = grant["grantId"].as_str().unwrap().to_owned();

    // The granted file reads back; the other one is outside the scope.
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &notes))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"original notes");
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &diary))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "outside-scope");

    // Another session of the same account never inherits session consent.
    let second = sessions.login("alice", password.clone()).unwrap();
    let response = app
        .clone()
        .oneshot(file_read_request(
            &second,
            &installation.to_string(),
            &notes,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "approval-required");

    // Durable write consent requires the account password.
    let durable = serde_json::json!({
        "permission": "rumahl.files.write",
        "resources": [notes],
        "validity": "durable",
    });
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            durable.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let wrong_password = serde_json::json!({
        "permission": "rumahl.files.write",
        "resources": [notes],
        "validity": "durable",
        "password": "not-the-password",
    });
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            wrong_password,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    let right_password = serde_json::json!({
        "permission": "rumahl.files.write",
        "resources": [notes],
        "validity": "durable",
        "password": password,
    });
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            right_password,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let durable_grant = json(response).await;
    assert_eq!(durable_grant["validity"], "durable");
    assert!(durable_grant["expiresAt"].is_null());
    let durable_grant_id = durable_grant["grantId"].as_str().unwrap().to_owned();

    // The app writes through its durable consent.
    let response = app
        .clone()
        .oneshot(file_write_request(
            &alice,
            &installation.to_string(),
            &notes,
            b"rewritten by the app",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = json(response).await;
    assert_eq!(value["outcome"], "allowed");
    assert_eq!(value["name"], "notes.txt");
    assert_eq!(
        files.read(user_id, &notes).unwrap().1,
        b"rewritten by the app".to_vec()
    );

    // A permission the app never requested cannot be consented to, and a
    // resource the user does not own cannot be granted.
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            serde_json::json!({
                "permission": "rumahl.files.read",
                "resources": ["doesnotexist0000000000000000000000ff"],
                "validity": "session",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(grants_request(&alice, &installation.to_string()))
        .await
        .unwrap();
    let value = json(response).await;
    assert_eq!(value["grants"].as_array().unwrap().len(), 2);

    // A restart keeps the durable consent and loses the session one.
    let restarted = gateway(root);
    let response = restarted
        .clone()
        .oneshot(file_write_request(
            &alice,
            &installation.to_string(),
            &notes,
            b"written after the restart",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        files.read(user_id, &notes).unwrap().1,
        b"written after the restart".to_vec()
    );
    let response = restarted
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &notes))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "approval-required");

    // Revoking the durable grant takes effect immediately.
    let response = restarted
        .clone()
        .oneshot(revoke_request(
            &alice,
            &installation.to_string(),
            &durable_grant_id,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let response = restarted
        .clone()
        .oneshot(file_write_request(
            &alice,
            &installation.to_string(),
            &notes,
            b"should not work",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "approval-required");

    // Session grants can be revoked within their session, too.
    let response = app
        .clone()
        .oneshot(revoke_request(
            &alice,
            &installation.to_string(),
            &session_grant_id,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &notes))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json(response).await["reason"], "approval-required");

    // Unknown installations and foreign apps are not addressable.
    let foreign = InstallationId::new().to_string();
    let response = app
        .clone()
        .oneshot(grants_request(&alice, &foreign))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn consent_bodies_are_validated_and_missing_files_report_absent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let accounts = root.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "Alice",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let sessions = LocalBrowserSessions::open(&accounts).unwrap();
    let alice = sessions.login("alice", password).unwrap();
    let installation = seed_files_app(root, "alice");

    let files = SqlitePersonalFiles::open(root.join("files.sqlite")).unwrap();
    let user_id = {
        let state = SqliteAccountStateRepository::open(&accounts)
            .unwrap()
            .load()
            .unwrap();
        *state
            .accounts()
            .accounts()
            .iter()
            .find(|user| user.username().as_str() == "alice")
            .unwrap()
            .user_id()
    };
    files
        .create(user_id, "root", "notes.txt", Some(b"notes".to_vec()))
        .unwrap();
    let notes = files.list(user_id, "root").unwrap()[0].id.clone();

    let app = gateway(root);

    // Unknown fields, missing origin, bad validity and empty resources never
    // reach the grant machinery.
    for (body, status) in [
        (
            serde_json::json!({"permission": "rumahl.files.read", "resources": [notes], "validity": "session", "extra": 1}),
            axum::http::StatusCode::BAD_REQUEST,
        ),
        (
            serde_json::json!({"permission": "rumahl.files.read", "resources": [], "validity": "session"}),
            axum::http::StatusCode::BAD_REQUEST,
        ),
        (
            serde_json::json!({"permission": "rumahl.files.read", "resources": [notes], "validity": "forever"}),
            axum::http::StatusCode::BAD_REQUEST,
        ),
        (
            serde_json::json!({"permission": "rumahl.files.read", "resources": [notes], "validity": "session", "expiresIn": 10}),
            axum::http::StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(issue_request(&alice, &installation.to_string(), body))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }

    // A read consent for a file id the user owns but that does not exist.
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            serde_json::json!({
                "permission": "rumahl.files.read",
                "resources": ["ffffffffffffffffffffffffffffffff"],
                "validity": "session",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

    // Garbage installation ids and keys never reach the provider.
    let response = app
        .clone()
        .oneshot(grants_request(&alice, "not-a-uuid"))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), ""))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

    // A granted-but-missing file reports absent, not denied: the consent is
    // fine, the document is gone.
    let response = app
        .clone()
        .oneshot(issue_request(
            &alice,
            &installation.to_string(),
            serde_json::json!({
                "permission": "rumahl.files.read",
                "resources": [notes],
                "validity": "session",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    files.delete(user_id, &notes).unwrap();
    let response = app
        .clone()
        .oneshot(file_read_request(&alice, &installation.to_string(), &notes))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

    // Writes need the octet-stream content type, like the personal files API.
    let request = axum::http::Request::builder()
        .method("PUT")
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.files-app/files/content?installationId={}&key={notes}",
            installation
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(vec![0u8]))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

#[tokio::test]
async fn channel_paths_resolve_only_the_callers_home() {
    use rumahl_platform_web::ChannelFiles;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let accounts = root.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "Alice",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let sessions = LocalBrowserSessions::open(&accounts).unwrap();
    let _alice = sessions.login("alice", password).unwrap();
    let installation = seed_files_app(root, "alice");
    let files = Arc::new(SqlitePersonalFiles::open(root.join("files.sqlite")).unwrap());
    let (user_id, session_id) = {
        let state = SqliteAccountStateRepository::open(&accounts)
            .unwrap()
            .load()
            .unwrap();
        let user = *state
            .accounts()
            .accounts()
            .iter()
            .find(|user| user.username().as_str() == "alice")
            .unwrap()
            .user_id();
        let session = *state
            .sessions()
            .sessions_for_user(&user)
            .next()
            .unwrap()
            .session_id();
        (user, session)
    };
    let identity = ShellIdentity {
        user_id,
        session_id,
    };
    files.create(user_id, "root", "Documents", None).unwrap();
    let documents = files.list(user_id, "root").unwrap().remove(0);
    files
        .create(user_id, &documents.id, "notes.txt", Some(b"hello".to_vec()))
        .unwrap();
    let expected = files
        .resolve_path(user_id, "Documents/notes.txt")
        .unwrap()
        .0;

    let provider = PersistentApps::open(
        &accounts,
        &root.join("platform.sqlite"),
        root.join("app-assets"),
    )
    .unwrap()
    .with_account_roles(&accounts)
    .unwrap()
    .with_personal_files(files.clone() as Arc<dyn PersonalFiles>);
    let origin = GatewayConfig::new(ORIGIN, root.join("ssr.sock"))
        .unwrap()
        .public_origin;
    let access = AppAccess::new(
        Arc::new(provider),
        &origin,
        &[],
        &["apps.rumahl.test".to_owned()],
        None,
    )
    .unwrap();

    // Both the internal absolute path and the client-neutral URI resolve to
    // the same store id; consent still applies to that id.
    assert_eq!(
        access
            .resolve(identity, installation, "/home/alice/Documents/notes.txt")
            .unwrap(),
        expected
    );
    assert_eq!(
        access
            .resolve(identity, installation, "rumahl://home/Documents/notes.txt")
            .unwrap(),
        expected
    );
    // Another account's home, an unowned area and a missing file all deny.
    assert!(
        access
            .resolve(identity, installation, "/home/bob/Documents/notes.txt")
            .is_err()
    );
    assert!(
        access
            .resolve(identity, installation, "/apps/com.rumahl/files-app")
            .is_err()
    );
    assert!(
        access
            .resolve(identity, installation, "/home/alice/missing.txt")
            .is_err()
    );
    assert!(
        access
            .resolve(identity, installation, "/home/alice/../etc/passwd")
            .is_err()
    );
}

#[tokio::test]
async fn a_policy_can_cap_consent_at_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let accounts = root.join("accounts.sqlite");
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    provision(
        &accounts,
        "alice",
        "Alice",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let sessions = LocalBrowserSessions::open(&accounts).unwrap();
    let alice = sessions.login("alice", password).unwrap();
    let installation = seed_files_app(root, "alice");
    let files = SqlitePersonalFiles::open(root.join("files.sqlite")).unwrap();
    let user_id = {
        let state = SqliteAccountStateRepository::open(&accounts)
            .unwrap()
            .load()
            .unwrap();
        *state
            .accounts()
            .accounts()
            .iter()
            .find(|user| user.username().as_str() == "alice")
            .unwrap()
            .user_id()
    };
    files
        .create(user_id, "root", "notes.txt", Some(b"hello".to_vec()))
        .unwrap();
    let notes = files.list(user_id, "root").unwrap().remove(0);

    // The policy store caps consent at the session, like a restart would see.
    let policies = rumahl_persistence_sqlite::SqlitePolicyStore::open(&accounts).unwrap();
    policies
        .set_assignment(
            rumahl_core::PolicySubject::Device,
            rumahl_core::PolicyKey::parse("rumahl.files.durable_grants").unwrap(),
            Some(rumahl_core::PolicyValue::Boolean(false)),
        )
        .unwrap();
    drop(policies);

    let app = gateway(root);
    // A durable consent is refused with a policy reason; a session consent
    // still works.
    let durable = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/v1/shell/apps/com.rumahl.files-app/grants?installationId={installation}"
                ))
                .header("cookie", format!("__Host-rumahl_session={}", &*alice))
                .header("origin", ORIGIN)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({
                        "permission": "rumahl.files.read",
                        "resources": [notes.id],
                        "validity": "durable"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(durable.status(), axum::http::StatusCode::FORBIDDEN);
    let bytes = http_body_util::BodyExt::collect(durable.into_body())
        .await
        .unwrap()
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["reason"], "durable-grants-disabled");

    let session = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/v1/shell/apps/com.rumahl.files-app/grants?installationId={installation}"
                ))
                .header("cookie", format!("__Host-rumahl_session={}", &*alice))
                .header("origin", ORIGIN)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({
                        "permission": "rumahl.files.read",
                        "resources": [notes.id],
                        "validity": "session"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session.status(), axum::http::StatusCode::OK);
}
