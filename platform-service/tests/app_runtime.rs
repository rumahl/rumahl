//! Router-level tests for the recorded app image environment and app storage.
use http_body_util::BodyExt;
use rumahl_core::*;
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteOsModeRepository};
use rumahl_platform_service::apps::PersistentApps;
use rumahl_platform_service::*;
use rumahl_platform_web::{
    AppAccess, BrowserSessions, GatewayConfig, GatewayState, InMemoryShellEvents, router,
};
use std::sync::Arc;
use tower::ServiceExt;

const ORIGIN: &str = "https://rumahl.test";
const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

/// Seeds one installed web app with a launch grant for the given user.
fn seed_app(root: &std::path::Path, username: &str) -> InstallationId {
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
    let manifest = AppManifest::new(
        AppId::parse("com.rumahl.notes").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Notes",
        runtime,
    )
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

fn gateway(root: &std::path::Path) -> axum::Router {
    let accounts = root.join("accounts.sqlite");
    let platform = root.join("platform.sqlite");
    let config = GatewayConfig::new(ORIGIN, root.join("ssr.sock")).unwrap();
    let provider = PersistentApps::open(&accounts, &platform, root.join("app-assets"))
        .unwrap()
        .with_account_roles(&accounts)
        .unwrap()
        .with_data_root(root.join("apps-data"))
        .unwrap()
        .with_storage_quota(1024 * 1024);
    router(GatewayState {
        host_files: None,
        weather_warnings: None,
        config: GatewayConfig::new(ORIGIN, root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &platform, "test", "en").unwrap(),
        browser_sessions: Some(Arc::new(LocalBrowserSessions::open(&accounts).unwrap())),
        admin: None,
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        audit: None,
        state_dir: None,
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: Some(Arc::new(
            AppAccess::new(
                Arc::new(provider),
                &config.public_origin,
                "apps.rumahl.test",
            )
            .unwrap(),
        )),
        oidc: None,
        files: None,
        workspace: None,
        preferences: None,
        os_mode: Some(Arc::new(
            SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap(),
        )),
        policies: None,
        platform_config: None,
    })
}

async fn body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn image_request(token: &str, installation: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.notes/image?installationId={installation}&device={DEVICE}"
        ))
        .header("cookie", format!("__Host-rumahl_session={token}"))
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
async fn the_recorded_image_environment_is_an_advanced_surface() {
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
    let installation = seed_app(root, "alice");

    // A recorded image environment with one credential-sounding entry.
    std::fs::create_dir_all(root.join("app-runtimes")).unwrap();
    std::fs::write(
        root.join("app-runtimes")
            .join(format!("{installation}.json")),
        serde_json::json!({
            "version": 1,
            "image": format!("sha256:{}", "a".repeat(64)),
            "environment": [
                {"name": "PATH", "value": "/usr/bin:/bin"},
                {"name": "RUMAHL_SECRET_TOKEN", "value": "baked-into-the-image"},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let app = gateway(root);

    // Unauthenticated reads require a session.
    let response = app
        .clone()
        .oneshot(image_request("invalid", &installation.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

    // The guided default keeps the recorded metadata closed.
    let response = app
        .clone()
        .oneshot(image_request(&alice, &installation.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    // Raising the device mode opens the view, without the secret entry.
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
    SqliteOsModeRepository::open(root.join("preferences.sqlite"))
        .unwrap()
        .save(
            user_id,
            BrowserProfileId::parse(DEVICE).unwrap(),
            0,
            PreferenceScope::Device,
            Some(OsMode::Advanced),
        )
        .unwrap();
    let response = app
        .clone()
        .oneshot(image_request(&alice, &installation.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = body(response).await;
    assert_eq!(value["id"], "com.rumahl.notes");
    assert_eq!(value["runtime"], "web");
    assert_eq!(value["image"], format!("sha256:{}", "a".repeat(64)));
    let entries = value["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "PATH");
    assert!(
        !serde_json::to_string(&value["entries"])
            .unwrap()
            .contains("SECRET")
    );

    // Unknown installations are not addressable.
    let foreign = InstallationId::new().to_string();
    let response = app
        .clone()
        .oneshot(image_request(&alice, &foreign))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

    // A fresh installation without a record reports its runtime and nothing
    // else, never a guess.
    let fresh = seed_app(root, "alice");
    let response = app
        .clone()
        .oneshot(image_request(&alice, &fresh.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = body(response).await;
    assert_eq!(value["runtime"], "web");
    assert!(value["image"].is_null());
    assert_eq!(value["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn app_storage_is_measured_and_gated_like_the_data_browser() {
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
    let _installation = seed_app(root, "alice");
    // The app's private data directory carries two files.
    let data = root.join("apps-data/com.rumahl.notes");
    std::fs::create_dir_all(data.join("state")).unwrap();
    std::fs::write(data.join("state/index.db"), vec![0_u8; 100]).unwrap();
    std::fs::write(data.join("log.txt"), vec![0_u8; 20]).unwrap();

    let app = gateway(root);
    let request = axum::http::Request::builder()
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.notes/storage?installationId={}&device={DEVICE}",
            _installation
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .body(axum::body::Body::empty())
        .unwrap();

    // Guided mode keeps storage diagnostics closed.
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

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
    SqliteOsModeRepository::open(root.join("preferences.sqlite"))
        .unwrap()
        .save(
            user_id,
            BrowserProfileId::parse(DEVICE).unwrap(),
            0,
            PreferenceScope::Device,
            Some(OsMode::Advanced),
        )
        .unwrap();

    let request = axum::http::Request::builder()
        .uri(format!(
            "/api/v1/shell/apps/com.rumahl.notes/storage?installationId={}&device={DEVICE}",
            _installation
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = body(response).await;
    assert_eq!(value["id"], "com.rumahl.notes");
    assert_eq!(value["usedBytes"], 120);
    assert_eq!(value["quotaBytes"], 1024 * 1024);
}
