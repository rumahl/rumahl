//! Router-level tests for host file browsing and the writable home area.
use http_body_util::BodyExt;
use rumahl_core::{
    AccountStateRepository, BrowserProfileId, OsMode, OsModeRepository, PreferenceScope,
};
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteOsModeRepository};
use rumahl_platform_service::host_files::LocalHostFiles;
use rumahl_platform_service::*;
use rumahl_platform_web::{
    BrowserSessions, GatewayConfig, GatewayState, HostFiles, InMemoryShellEvents, router,
};
use std::sync::Arc;
use tower::ServiceExt;

const ORIGIN: &str = "https://rumahl.test";
const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

fn gateway(root: &std::path::Path, host: Arc<dyn HostFiles>) -> axum::Router {
    let accounts = root.join("accounts.sqlite");
    let platform = root.join("platform.sqlite");
    router(GatewayState {
        host_files: Some(host),
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
        apps: None,
        oidc: None,
        files: None,
        workspace: None,
        preferences: None,
        os_mode: Some(Arc::new(
            SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap(),
        )),
        platform_config: None,
    })
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn host_browsing_and_home_writes_respect_advanced_mode_and_the_home_area() {
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

    // Home + apps roots; alice's home carries one document.
    let home_root = root.join("home");
    std::fs::create_dir_all(home_root.join("alice/Documents")).unwrap();
    std::fs::write(home_root.join("alice/Documents/notes.txt"), b"mine").unwrap();
    std::fs::create_dir_all(root.join("apps/com.example.notes")).unwrap();
    let host: Arc<dyn HostFiles> = Arc::new(
        LocalHostFiles::new(root.join("apps"), root.join("system"))
            .with_homes(home_root.clone(), accounts.clone()),
    );
    let app = gateway(root, host);

    let listing = |token: &str, area: &str| {
        axum::http::Request::builder()
            .uri(format!(
                "/api/v1/shell/fs?area={area}&path=&device={DEVICE}"
            ))
            .header("cookie", format!("__Host-rumahl_session={token}"))
            .body(axum::body::Body::empty())
            .unwrap()
    };
    let write = |token: &str, path: &str, body: &'static [u8]| {
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/shell/fs/content?area=home&path={path}&device={DEVICE}"
            ))
            .header("cookie", format!("__Host-rumahl_session={token}"))
            .header("origin", ORIGIN)
            .header("content-type", "application/octet-stream")
            .body(axum::body::Body::from(body.to_vec()))
            .unwrap()
    };

    // Unauthenticated requests get nothing.
    let response = app
        .clone()
        .oneshot(listing("invalid", "home"))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

    // The user's own home is not a system surface: it stays visible in guided
    // mode, while installed apps and the system tree remain advanced-only.
    let response = app.clone().oneshot(listing(&alice, "home")).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let response = app.clone().oneshot(listing(&alice, "apps")).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    // Advanced mode opens the read-only areas.
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
    let response = app.clone().oneshot(listing(&alice, "home")).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = json(response).await;
    assert_eq!(value["area"], "home");
    assert_eq!(value["entries"][0]["name"], "Documents");
    let response = app.clone().oneshot(listing(&alice, "apps")).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = json(response).await;
    assert_eq!(value["entries"][0]["name"], "com.example.notes");

    // Home writes need the origin and land contained in the home directory.
    let no_origin = axum::http::Request::builder()
        .method("PUT")
        .uri(format!(
            "/api/v1/shell/fs/content?area=home&path=Documents/notes.txt&device={DEVICE}"
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .header("content-type", "application/octet-stream")
        .body(axum::body::Body::from(b"x".to_vec()))
        .unwrap();
    let response = app.clone().oneshot(no_origin).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    let response = app
        .clone()
        .oneshot(write(&alice, "Documents/notes.txt", b"rewritten"))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read(home_root.join("alice/Documents/notes.txt")).unwrap(),
        b"rewritten"
    );

    // The apps area stays read-only even in advanced mode.
    let apps_write = axum::http::Request::builder()
        .method("PUT")
        .uri(format!(
            "/api/v1/shell/fs/content?area=apps&path=com.example.notes/x&device={DEVICE}"
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .header("origin", ORIGIN)
        .header("content-type", "application/octet-stream")
        .body(axum::body::Body::from(b"x".to_vec()))
        .unwrap();
    let response = app.clone().oneshot(apps_write).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    // Creating and deleting entries stays inside the home.
    let create = |token: &str, name: &str| {
        axum::http::Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v1/shell/fs?area=home&path=&name={name}&directory=true&device={DEVICE}"
            ))
            .header("cookie", format!("__Host-rumahl_session={token}"))
            .header("origin", ORIGIN)
            .body(axum::body::Body::empty())
            .unwrap()
    };
    let response = app
        .clone()
        .oneshot(create(&alice, "Projects"))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    assert!(home_root.join("alice/Projects").is_dir());
    // Traversal names are refused before the filesystem is touched.
    let response = app
        .clone()
        .oneshot(create(&alice, "..%2Fescape"))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

    let remove = axum::http::Request::builder()
        .method("DELETE")
        .uri(format!(
            "/api/v1/shell/fs?area=home&path=Projects&device={DEVICE}"
        ))
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .header("origin", ORIGIN)
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(remove).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    assert!(!home_root.join("alice/Projects").exists());
}
