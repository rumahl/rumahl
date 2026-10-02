#[path = "support/app_fixture.rs"]
mod fixture;
use rumahl_core::*;
use rumahl_persistence_sqlite::SqliteSnapshotRepository;
use rumahl_platform_service::{apps::PersistentApps, *};
use rumahl_platform_web::{AppAccessError, AppProvider, BrowserSessions};

#[test]
fn live_permissions_sessions_and_filesystem_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let accounts = root.join("accounts.sqlite");
    let platform = root.join("platform.sqlite");
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
    provision(
        &accounts,
        "bob",
        "Bob",
        password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let sessions = LocalBrowserSessions::open(&accounts).unwrap();
    let alice_token = sessions.login("alice", password.clone()).unwrap();
    let bob_token = sessions.login("bob", password).unwrap();
    let backend = shell_backend(&accounts, &platform, "test", "en").unwrap();
    let alice = backend.authenticate(&alice_token).unwrap();
    let bob = backend.authenticate(&bob_token).unwrap();
    let installation = fixture::seed(root, "alice");
    let provider = PersistentApps::open(&accounts, &platform, root.join("app-assets")).unwrap();
    assert_eq!(provider.catalog(alice).unwrap().len(), 1);
    assert!(provider.catalog(alice).unwrap()[0].launchable);
    assert!(provider.catalog(bob).unwrap().is_empty());
    assert!(
        provider
            .asset(bob, installation, "frontend/index.html")
            .is_err()
    );
    assert_eq!(
        provider
            .asset(alice, installation, "frontend/main.js")
            .unwrap()
            .content_type,
        "text/javascript; charset=utf-8"
    );
    let assets = root.join("app-assets").join(installation.to_string());
    std::fs::write(assets.join("secret.json"), "{}").unwrap();
    std::os::unix::fs::symlink(
        assets.join("secret.json"),
        assets.join("frontend/link.json"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&assets, assets.join("frontend/escape")).unwrap();
    std::fs::File::create(assets.join("frontend/large.js"))
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    for path in [
        "secret.json",
        "frontend/../secret.json",
        "frontend/link.json",
        "frontend/escape/secret.json",
        "frontend/large.js",
        "frontend/no.exe",
    ] {
        assert!(
            matches!(
                provider.asset(alice, installation, path),
                Err(AppAccessError::Denied)
            ),
            "{path}"
        );
    }
    let original = SqliteSnapshotRepository::open(&platform)
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    fixture::revoke(root);
    assert!(provider.catalog(alice).unwrap().is_empty());
    assert!(
        provider
            .asset(alice, installation, "frontend/index.html")
            .is_err()
    );
    SqliteSnapshotRepository::open(&platform)
        .unwrap()
        .store(&original)
        .unwrap();
    assert!(
        provider
            .asset(alice, installation, "frontend/index.html")
            .is_ok()
    );
    // A reinstall has a different identity even when the logical app ID is unchanged.
    let mut state = PlatformState::new();
    AppLifecycle::new()
        .install(original.installed_apps()[0].manifest().clone(), &mut state)
        .unwrap();
    SqliteSnapshotRepository::open(&platform)
        .unwrap()
        .store(&PlatformSnapshot::new(
            original.version(),
            PlatformSnapshot::capture(&state, &InMemoryGrantStore::new())
                .installed_apps()
                .to_vec(),
            original.grants().to_vec(),
        ))
        .unwrap();
    assert!(provider.catalog(alice).unwrap().is_empty());
    assert!(
        provider
            .asset(alice, installation, "frontend/index.html")
            .is_err()
    );
    SqliteSnapshotRepository::open(&platform)
        .unwrap()
        .store(&original)
        .unwrap();
    sessions.logout(&alice_token).unwrap();
    assert!(provider.catalog(alice).is_err());
    assert!(
        provider
            .asset(alice, installation, "frontend/index.html")
            .is_err()
    );
}

#[tokio::test]
async fn catalog_and_launch_require_session_origin_and_current_installation() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use rumahl_platform_web::{
        AppAccess, GatewayConfig, GatewayState, InMemoryShellEvents, router,
    };
    use std::sync::Arc;
    use tower::ServiceExt;
    let dir = tempfile::tempdir().unwrap();
    let accounts = dir.path().join("accounts.sqlite");
    let platform = dir.path().join("platform.sqlite");
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
    let sessions = Arc::new(LocalBrowserSessions::open(&accounts).unwrap());
    let token = sessions.login("alice", password).unwrap();
    let installation = fixture::seed(dir.path(), "alice");
    let config = GatewayConfig::new("https://rumahl.test", dir.path().join("ssr.sock")).unwrap();
    let apps = Arc::new(
        AppAccess::new(
            Arc::new(
                PersistentApps::open(&accounts, &platform, dir.path().join("app-assets")).unwrap(),
            ),
            &config.public_origin,
            "apps.rumahl.test",
        )
        .unwrap(),
    );
    let app = router(GatewayState {
        config,
        backend: shell_backend(&accounts, &platform, "test", "en").unwrap(),
        browser_sessions: Some(sessions.clone()),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: Some(apps.clone()),
        preferences: None,
        workspace: None,
        files: None,
        oidc: None,
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/shell/apps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/shell/apps")
                .header(
                    "cookie",
                    format!("__Host-rumahl_session={}", token.as_str()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let catalog: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(catalog["apps"][0]["id"], "com.rumahl.host-test");
    let body = serde_json::json!({"installationId": installation.to_string()}).to_string();
    for origin in [None, Some("null"), Some("https://evil.test")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/shell/apps/com.rumahl.host-test/launch")
            .header(
                "cookie",
                format!("__Host-rumahl_session={}", token.as_str()),
            )
            .header("content-type", "application/json");
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::from(body.clone())).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/api/v1/shell/apps/com.rumahl.host-test/launch")
            .header(
                "cookie",
                format!("__Host-rumahl_session={}", token.as_str()),
            )
            .header("content-type", "application/json")
            .header("origin", "https://rumahl.test")
            .body(Body::from(body.clone()))
            .unwrap()
    };
    let response = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    fixture::revoke(dir.path());
    assert_eq!(
        app.clone().oneshot(request()).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    sessions.logout(&token).unwrap();
    assert_eq!(
        app.oneshot(request()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
