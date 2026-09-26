use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rumahl_persistence_sqlite::SqliteShellPreferences;
use rumahl_platform_service::*;
use rumahl_platform_web::{
    BrowserSessions, GatewayConfig, GatewayState, InMemoryShellEvents, router,
};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn preferences_are_session_scoped_csrf_protected_and_revision_checked() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let accounts = root.join("accounts.sqlite");
    let sessions = Arc::new(LocalBrowserSessions::open(&accounts).unwrap());
    let password = rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string();
    for name in ["alice", "bob"] {
        provision(
            &accounts,
            name,
            name,
            password.clone(),
            LocalPasswordBlocklist::default(),
        )
        .unwrap();
    }
    let alice = sessions.login("alice", password.clone()).unwrap();
    let bob = sessions.login("bob", password).unwrap();
    let app = router(GatewayState {
        config: GatewayConfig::new("https://rumahl.test", root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &root.join("platform.sqlite"), "test", "en").unwrap(),
        browser_sessions: Some(sessions.clone()),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: None,
        oidc: None,
        preferences: Some(Arc::new(
            SqliteShellPreferences::open(root.join("preferences.sqlite")).unwrap(),
        )),
    });
    let endpoint = "/api/v1/shell/preferences?device=00000000-0000-4000-8000-000000000001";
    let request = |token: Option<&str>, origin: Option<&str>, body: Option<serde_json::Value>| {
        let mut r =
            Request::builder()
                .uri(endpoint)
                .method(if body.is_some() { "PUT" } else { "GET" });
        if let Some(token) = token {
            r = r.header("cookie", format!("__Host-rumahl_session={token}"));
        }
        if let Some(origin) = origin {
            r = r.header("origin", origin);
        }
        r.header("content-type", "application/json")
            .body(Body::from(body.map_or(String::new(), |v| v.to_string())))
            .unwrap()
    };
    let update = serde_json::json!({"settingsVersion":1,"revision":0,"scope":"user","key":"shell.mode","value":"launcher"});
    assert_eq!(
        app.clone()
            .oneshot(request(None, None, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for origin in [None, Some("null"), Some("https://evil.test")] {
        assert_eq!(
            app.clone()
                .oneshot(request(Some(&alice), origin, Some(update.clone())))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let mut forged = update.clone();
    forged["ownerId"] = "other".into();
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(forged)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = app
        .clone()
        .oneshot(request(
            Some(&alice),
            Some("https://rumahl.test"),
            Some(update.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["effective"]["shellMode"], "launcher");
    assert_eq!(value["revision"], 1);
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(update)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let response = app
        .clone()
        .oneshot(request(Some(&bob), None, None))
        .await
        .unwrap();
    let bob_value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(bob_value["effective"]["shellMode"], "desktop");
    assert_ne!(bob_value["ownerId"], value["ownerId"]);
    sessions.logout(&alice).unwrap();
    assert_eq!(
        app.oneshot(request(Some(&alice), None, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
