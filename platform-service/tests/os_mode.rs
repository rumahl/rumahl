use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rumahl_persistence_sqlite::{SqliteOsModeRepository, SqliteShellPreferences};
use rumahl_platform_service::*;
use rumahl_platform_web::{
    BrowserSessions, GatewayConfig, GatewayState, InMemoryShellEvents, router,
};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn raising_the_mode_requires_reauthentication() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let accounts = root.join("accounts.sqlite");
    let sessions = Arc::new(LocalBrowserSessions::open(&accounts).unwrap());
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
    let alice = sessions.login("alice", password.clone()).unwrap();

    let app = router(GatewayState {
        host_files: None,
        config: GatewayConfig::new("https://rumahl.test", root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &root.join("platform.sqlite"), "test", "en").unwrap(),
        browser_sessions: Some(sessions.clone()),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: None,
        oidc: None,
        files: None,
        workspace: Some(Arc::new(
            SqliteShellPreferences::open(root.join("preferences.sqlite")).unwrap(),
        )),
        preferences: Some(Arc::new(
            SqliteShellPreferences::open(root.join("preferences.sqlite")).unwrap(),
        )),
        os_mode: Some(Arc::new(
            SqliteOsModeRepository::open(root.join("preferences.sqlite")).unwrap(),
        )),
    });

    let endpoint = "/api/v1/shell/os-mode?device=00000000-0000-4000-8000-000000000001";
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
    let put = |mode: serde_json::Value, revision: u64, password: Option<&str>| {
        let mut body = serde_json::json!({
            "settingsVersion": 1,
            "revision": revision,
            "scope": "user",
            "mode": mode,
        });
        if let Some(password) = password {
            body["password"] = serde_json::Value::String(password.to_owned());
        }
        body
    };
    let effective =
        |value: &serde_json::Value| value["effective"]["osMode"].as_str().unwrap().to_owned();

    // Unauthenticated reads require a session.
    assert_eq!(
        app.clone()
            .oneshot(request(None, None, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // A fresh account is guided.
    let response = app
        .clone()
        .oneshot(request(Some(&alice), None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(effective(&value), "guided");
    assert_eq!(value["revision"], 0);

    // Missing origin and unknown fields are rejected.
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://evil.test"),
                Some(put("developer".into(), 0, Some(&password)))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut extra = put("guided".into(), 0, None);
    extra["ownerId"] = "attacker".into();
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(extra)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    // Raising without a password is rejected.
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(put("developer".into(), 0, None))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    // Raising with the wrong password is forbidden.
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(put("developer".into(), 0, Some("wrong-password")))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    // Raising with the correct password succeeds.
    let response = app
        .clone()
        .oneshot(request(
            Some(&alice),
            Some("https://rumahl.test"),
            Some(put("developer".into(), 0, Some(&password))),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(effective(&value), "developer");
    assert_eq!(value["revision"], 1);

    // Lowering does not require a password.
    let response = app
        .clone()
        .oneshot(request(
            Some(&alice),
            Some("https://rumahl.test"),
            Some(put("guided".into(), 1, None)),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(effective(&value), "guided");
    assert_eq!(value["revision"], 2);

    // A stale revision conflicts.
    assert_eq!(
        app.clone()
            .oneshot(request(
                Some(&alice),
                Some("https://rumahl.test"),
                Some(put("guided".into(), 1, None))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}
