use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rumahl_core::AuditLog;
use rumahl_persistence_sqlite::SqliteAuditLog;
use rumahl_platform_service::*;
use rumahl_platform_web::{GatewayConfig, GatewayState, InMemoryShellEvents, router};
use std::{path::Path, sync::Arc};
use tower::ServiceExt;

fn password() -> String {
    rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string()
}

fn app(root: &Path) -> Router {
    let accounts = root.join("accounts.sqlite");
    let audit: Arc<dyn AuditLog> =
        Arc::new(SqliteAuditLog::open(root.join("audit.sqlite")).unwrap());
    let admin = Arc::new(LocalAdministration::open(&accounts, Some(audit.clone())).unwrap());
    router(GatewayState {
        host_files: None,
        weather_warnings: None,
        config: GatewayConfig::new("https://rumahl.test", root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &root.join("platform.sqlite"), "build-test", "en")
            .unwrap(),
        browser_sessions: Some(Arc::new(LocalBrowserSessions::open(&accounts).unwrap())),
        admin: Some(admin),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        audit: Some(audit),
        state_dir: None,
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: None,
        preferences: None,
        os_mode: None,
        workspace: None,
        files: None,
        oidc: None,
    })
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", format!("__Host-rumahl_session={cookie}"));
    }
    if method != "GET" {
        request = request.header("origin", "https://rumahl.test");
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    body.map_or(String::new(), |value| value.to_string()),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn login(app: &Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("origin", "https://rumahl.test")
                .body(Body::from(format!(
                    "username={username}&password={password}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("__Host-rumahl_session=")
        .to_owned()
}

#[tokio::test]
async fn administrators_manage_users_roles_sessions_and_audit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let alice_password = password();
    let bob_password = password();
    let charlie_password = password();
    let db = root.join("accounts.sqlite");
    for (name, display, password) in [
        ("alice", "Alice", &alice_password),
        ("bob", "Bob", &bob_password),
    ] {
        provision(
            &db,
            name,
            display,
            password.clone(),
            LocalPasswordBlocklist::default(),
        )
        .unwrap();
    }
    let app = app(root);
    let alice = login(&app, "alice", &alice_password).await;

    let (status, snapshot) = call(&app, "GET", "/api/v1/shell/snapshot", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["user"]["role"], "owner");

    let (status, users) = call(&app, "GET", "/api/v1/admin/users", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users["users"].as_array().unwrap().len(), 2);

    let owner_id = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["role"] == "owner")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = call(&app, "POST", "/api/v1/admin/users", Some(&alice), Some(serde_json::json!({
        "username": "second-owner", "displayName": "Second Owner", "password": password(), "role": "owner"
    }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{owner_id}"),
        Some(&alice),
        Some(serde_json::json!({"role": "administrator"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, charlie) = call(&app, "POST", "/api/v1/admin/users", Some(&alice), Some(serde_json::json!({
        "username": "charlie", "displayName": "Charlie", "password": charlie_password, "role": "user"
    }))).await;
    assert_eq!(status, StatusCode::OK);
    let charlie_id = charlie["id"].as_str().unwrap().to_owned();
    assert_eq!(charlie["role"], "user");

    let (status, bob_user) = call(&app, "PATCH", "/api/v1/admin/users", Some(&alice), None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(bob_user.is_null());

    let bob_id = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "bob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, updated) = call(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{bob_id}"),
        Some(&alice),
        Some(serde_json::json!({"role": "administrator"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["role"], "administrator");

    let bob = login(&app, "bob", &bob_password).await;
    let (status, sessions) = call(&app, "GET", "/api/v1/admin/sessions", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sessions["sessions"].as_array().unwrap().len() >= 2);

    let charlie_password = rumahl_core::UserId::new().to_string();
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/admin/users/{charlie_id}/password"),
        Some(&alice),
        Some(serde_json::json!({"password": charlie_password})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let charlie = login(&app, "charlie", &charlie_password).await;

    let (status, renamed) = call(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{charlie_id}"),
        Some(&alice),
        Some(serde_json::json!({"username": "charlie-two", "displayName": "Charlie Two"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["username"], "charlie-two");
    assert_eq!(renamed["displayName"], "Charlie Two");

    let (status, _) = call(&app, "GET", "/api/v1/admin/users", Some(&charlie), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, snapshot) = call(&app, "GET", "/api/v1/shell/snapshot", Some(&bob), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["user"]["role"], "administrator");

    let (status, audit) = call(
        &app,
        "GET",
        "/api/v1/admin/audit?limit=20",
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!audit["events"].as_array().unwrap().is_empty());

    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/api/v1/admin/users/{bob_id}"),
        Some(&alice),
        Some(serde_json::json!({"role": "user"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "GET", "/api/v1/admin/users", Some(&bob), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let sessions = call(&app, "GET", "/api/v1/admin/sessions", Some(&alice), None)
        .await
        .1;
    let bob_session = sessions["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["userId"].as_str() == Some(&bob_id) && session["active"] == true)
        .unwrap()["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/admin/sessions/{bob_session}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "GET", "/api/v1/shell/snapshot", Some(&bob), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{owner_id}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{charlie_id}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, users) = call(&app, "GET", "/api/v1/admin/users", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        users["users"]
            .as_array()
            .unwrap()
            .iter()
            .all(|user| user["username"] != "charlie-two")
    );
}
