//! Router-level tests for the extended platform configuration view.
use http_body_util::BodyExt;
use rumahl_core::{AccountStateRepository, BrowserProfileId, OsMode, OsModeRepository};
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteOsModeRepository};
use rumahl_platform_service::*;
use rumahl_platform_web::{
    BrowserSessions, GatewayConfig, GatewayState, InMemoryShellEvents, PlatformConfig,
    PlatformConfigEntry, router,
};
use std::sync::Arc;
use tower::ServiceExt;

const ORIGIN: &str = "https://rumahl.test";
const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

fn gateway(root: &std::path::Path, config: Option<PlatformConfig>) -> axum::Router {
    let accounts = root.join("accounts.sqlite");
    let platform = root.join("platform.sqlite");
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
        apps: None,
        oidc: None,
        files: None,
        workspace: None,
        preferences: None,
        os_mode: Some(Arc::new(
            SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap(),
        )),
        policies: None,
        platform_config: config.map(Arc::new),
    })
}

async fn body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn platform_config_is_an_advanced_surface() {
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

    let config = PlatformConfig::from_entries(vec![
        PlatformConfigEntry {
            name: "RUMAHL_PUBLIC_ORIGIN".into(),
            value: ORIGIN.into(),
        },
        PlatformConfigEntry {
            name: "RUMAHL_SECRET_KEY_ID".into(),
            value: "never shown".into(),
        },
    ]);
    let app = gateway(root, Some(config));

    let request = |token: &str| {
        axum::http::Request::builder()
            .uri(format!("/api/v1/shell/config?device={DEVICE}"))
            .header("cookie", format!("__Host-rumahl_session={token}"))
            .body(axum::body::Body::empty())
            .unwrap()
    };

    // Unauthenticated reads require a session.
    let response = app.clone().oneshot(request("invalid")).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

    // The guided default keeps the extended configuration closed.
    let response = app.clone().oneshot(request(&alice)).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

    // Raising the device mode to advanced opens the view.
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
    let repository = SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap();
    repository
        .save(
            user_id,
            BrowserProfileId::parse(DEVICE).unwrap(),
            0,
            rumahl_core::PreferenceScope::Device,
            Some(OsMode::Advanced),
        )
        .unwrap();
    let response = app.clone().oneshot(request(&alice)).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let value = body(response).await;
    assert_eq!(value["configVersion"], 1);
    assert_eq!(value["entries"].as_array().unwrap().len(), 1);
    assert_eq!(value["entries"][0]["name"], "RUMAHL_PUBLIC_ORIGIN");
    assert_eq!(value["entries"][0]["value"], ORIGIN);

    // Bad device ids never reach the mode store.
    let bad = axum::http::Request::builder()
        .uri("/api/v1/shell/config?device=not-a-uuid")
        .header("cookie", format!("__Host-rumahl_session={}", &*alice))
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(bad).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

    // Without startup-captured configuration the view is unavailable, not empty.
    let bare = gateway(root, None);
    let response = bare.oneshot(request(&alice)).await.unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
}
