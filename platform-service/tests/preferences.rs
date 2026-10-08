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
        files: Some(Arc::new(
            rumahl_persistence_sqlite::SqlitePersonalFiles::open(root.join("files.sqlite"))
                .unwrap(),
        )),
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
    // New APIs use the same browser session boundary and never accept an owner ID.
    let send =
        |method: &str, uri: &str, token: &str, origin: &str, content: &str, body: Vec<u8>| {
            Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", format!("__Host-rumahl_session={token}"))
                .header("origin", origin)
                .header("content-type", content)
                .body(Body::from(body))
                .unwrap()
        };
    let workspace = "/api/v1/shell/workspace?device=00000000-0000-4000-8000-000000000001";
    let layout = serde_json::json!({"revision":0,"scope":"user","value":{"version":1,"windows":[],"folders":[]}}).to_string().into_bytes();
    assert_eq!(
        app.clone()
            .oneshot(send(
                "PUT",
                workspace,
                &alice,
                "https://evil.test",
                "application/json",
                layout.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(send(
                "PUT",
                workspace,
                &alice,
                "https://rumahl.test",
                "application/json",
                layout.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(send(
                "PUT",
                workspace,
                &alice,
                "https://rumahl.test",
                "application/json",
                layout
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let invalid=serde_json::json!({"revision":1,"scope":"user","value":{"version":1,"windows":[],"folders":[{"id":"a","name":"A","apps":["same","same"]}]}}).to_string().into_bytes();
    assert_eq!(
        app.clone()
            .oneshot(send(
                "PUT",
                workspace,
                &alice,
                "https://rumahl.test",
                "application/json",
                invalid
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let upload = "/api/v1/files?parent=root&name=test.html&directory=false";
    assert_eq!(
        app.clone()
            .oneshot(send(
                "POST",
                upload,
                &alice,
                "https://evil.test",
                "application/octet-stream",
                vec![1]
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(send(
                "POST",
                upload,
                &alice,
                "https://rumahl.test",
                "application/octet-stream",
                b"<script>alert(1)</script>".to_vec()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let list = app
        .clone()
        .oneshot(send(
            "GET",
            "/api/v1/files?parent=root",
            &alice,
            "https://rumahl.test",
            "",
            vec![],
        ))
        .await
        .unwrap();
    let list: serde_json::Value =
        serde_json::from_slice(&list.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let download = format!(
        "/api/v1/files/content?id={}",
        list[0]["id"].as_str().unwrap()
    );
    assert_eq!(
        app.clone()
            .oneshot(send(
                "GET",
                &download,
                &bob,
                "https://rumahl.test",
                "",
                vec![]
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let file = app
        .clone()
        .oneshot(send(
            "GET",
            &download,
            &alice,
            "https://rumahl.test",
            "",
            vec![],
        ))
        .await
        .unwrap();
    assert_eq!(file.headers()["content-type"], "application/octet-stream");
    assert!(
        file.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(file.headers()["cache-control"], "private, no-store");
    sessions.logout(&alice).unwrap();
    assert_eq!(
        app.oneshot(request(Some(&alice), None, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
