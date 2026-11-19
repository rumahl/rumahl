//! Router-level tests for the policy system: catalog, assignments, groups and
//! the three enforcement points (advanced mode, personal quota, cross-user
//! home access).
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

const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

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
    let admin = Arc::new(
        LocalAdministration::open(&accounts, Some(audit.clone()))
            .unwrap()
            .with_home_root(root.join("home")),
    );
    let policies: Arc<dyn rumahl_platform_web::PolicyEngine> = Arc::new(
        rumahl_platform_service::policy_engine::LocalPolicyEngine::open(&accounts).unwrap(),
    );
    let files: Arc<dyn rumahl_core::PersonalFiles> = Arc::new(
        rumahl_persistence_sqlite::HomePersonalFiles::open(
            root.join("home"),
            &accounts,
            root.join("files-index.sqlite"),
        )
        .unwrap(),
    );
    router(GatewayState {
        host_files: Some(Arc::new(
            rumahl_platform_service::host_files::LocalHostFiles::new(
                root.join("apps"),
                root.join("rumahl"),
            )
            .with_homes(root.join("home"), accounts.clone()),
        )),
        weather_warnings: None,
        config: GatewayConfig::new("https://rumahl.test", root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &root.join("platform.sqlite"), "build-test", "en")
            .unwrap(),
        browser_sessions: Some(Arc::new(LocalBrowserSessions::open(&accounts).unwrap())),
        admin: Some(admin),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        audit: Some(audit),
        state_dir: None,
        platform_config: None,
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: None,
        preferences: None,
        os_mode: Some(Arc::new(
            rumahl_persistence_sqlite::SqliteOsModeRepository::open(
                root.join("preferences.sqlite"),
            )
            .unwrap(),
        )),
        policies: Some(policies),
        workspace: None,
        files: Some(files),
        oidc: None,
    })
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

async fn json(
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

async fn raw(app: &Router, method: &str, path: &str, cookie: &str, body: Vec<u8>) -> StatusCode {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/octet-stream")
                .header("origin", "https://rumahl.test")
                .header("cookie", format!("__Host-rumahl_session={cookie}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

/// Seeds an owner (`alice`), a normal user (`bob`) and both home directories.
async fn seeded(root: &Path) -> (Router, String, String) {
    let accounts = root.join("accounts.sqlite");
    let alice_password = password();
    let bob_password = password();
    provision(
        &accounts,
        "alice",
        "Alice",
        alice_password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    provision(
        &accounts,
        "bob",
        "Bob",
        bob_password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    for name in ["alice", "bob"] {
        rumahl_platform_service::user_homes::ensure_user_home(&root.join("home"), name).unwrap();
    }
    std::fs::create_dir_all(root.join("home/bob/Documents")).unwrap();
    std::fs::write(root.join("home/bob/Documents/secret.txt"), b"bob's secret").unwrap();
    let router = app(root);
    let alice = login(&router, "alice", &alice_password).await;
    let bob = login(&router, "bob", &bob_password).await;
    (router, alice, bob)
}

#[tokio::test]
async fn administrators_manage_groups_and_assignments() {
    let temp = tempfile::tempdir().unwrap();
    let (app, alice, bob) = seeded(temp.path()).await;

    // Only administrators reach the policy surface.
    let (status, _) = json(&app, "GET", "/api/v1/admin/policies", Some(&bob), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The catalog is complete and assignments start empty.
    let (status, value) = json(&app, "GET", "/api/v1/admin/policies", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    let keys: Vec<&str> = value["policies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|definition| definition["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        vec![
            "rumahl.os.advanced_mode",
            "rumahl.home.cross_user_read",
            "rumahl.files.quota_bytes"
        ]
    );
    assert_eq!(value["assignments"].as_array().unwrap().len(), 0);

    // Unknown keys, wrong types and out-of-range numbers are refused.
    for body in [
        serde_json::json!({"subject": "device", "key": "rumahl.os.unknown", "value": true}),
        serde_json::json!({"subject": "device", "key": "rumahl.os.advanced_mode", "value": 1}),
        serde_json::json!({"subject": "device", "key": "rumahl.files.quota_bytes", "value": 1}),
        serde_json::json!({"subject": "device", "key": "rumahl.files.quota_bytes", "value": 1_000_000_000_000_000_000_u64}),
        serde_json::json!({"subject": "stranger:1", "key": "rumahl.os.advanced_mode", "value": true}),
    ] {
        let (status, _) = json(
            &app,
            "PUT",
            "/api/v1/admin/policies",
            Some(&alice),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // Groups: create, membership, assignments that reach members.
    let (status, group) = json(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&alice),
        Some(serde_json::json!({"name": "engineering"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let group_id = group["id"].as_str().unwrap().to_owned();
    // Duplicate names (case-insensitive) conflict.
    let (status, _) = json(
        &app,
        "POST",
        "/api/v1/admin/groups",
        Some(&alice),
        Some(serde_json::json!({"name": "Engineering"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Bob's id from the users list.
    let (_, users) = json(&app, "GET", "/api/v1/admin/users", Some(&alice), None).await;
    let bob_id = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "bob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, updated) = json(
        &app,
        "PUT",
        &format!("/api/v1/admin/groups/{group_id}/members"),
        Some(&alice),
        Some(serde_json::json!({"add": [bob_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["members"].as_array().unwrap().len(), 1);

    // A group assignment for the quota policy reaches bob through membership.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": format!("group:{group_id}"),
            "key": "rumahl.files.quota_bytes",
            "value": 16 * 1024 * 1024
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, value) = json(&app, "GET", "/api/v1/admin/policies", Some(&alice), None).await;
    assert_eq!(value["assignments"].as_array().unwrap().len(), 1);

    // Clearing the assignment works, deleting the group removes it too.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": format!("group:{group_id}"),
            "key": "rumahl.files.quota_bytes",
            "value": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = json(
        &app,
        "DELETE",
        &format!("/api/v1/admin/groups/{group_id}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, groups) = json(&app, "GET", "/api/v1/admin/groups", Some(&alice), None).await;
    assert_eq!(groups["groups"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn the_personal_quota_is_enforced_for_shell_uploads() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _alice, bob) = seeded(temp.path()).await;

    // A tiny budget for the device: one small file fits, a second does not.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&_alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.files.quota_bytes",
            "value": 16 * 1024 * 1024
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let status = raw(
        &app,
        "POST",
        "/api/v1/files?parent=root&name=first.txt&directory=false",
        &bob,
        b"first".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Eight bytes fit; sixteen megabytes and one byte do not.
    let status = raw(
        &app,
        "POST",
        "/api/v1/files?parent=root&name=second.txt&directory=false",
        &bob,
        b"second".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let oversize = vec![0_u8; 16 * 1024 * 1024 - 10];
    let status = raw(
        &app,
        "POST",
        "/api/v1/files?parent=root&name=huge.txt&directory=false",
        &bob,
        oversize,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn a_policy_can_close_advanced_mode() {
    let temp = tempfile::tempdir().unwrap();
    let (app, alice, bob) = seeded(temp.path()).await;

    // Closing the advanced-mode policy for bob forbids raising the mode…
    let (_, users) = json(&app, "GET", "/api/v1/admin/users", Some(&alice), None).await;
    let bob_id = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "bob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": format!("user:{bob_id}"),
            "key": "rumahl.os.advanced_mode",
            "value": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = json(
        &app,
        "PUT",
        &format!("/api/v1/shell/os-mode?device={DEVICE}"),
        Some(&bob),
        Some(serde_json::json!({
            "settingsVersion": 1, "revision": 0, "scope": "user", "mode": "advanced",
            "password": "any-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // …and closes the apps area even for a stored advanced mode.
    let (status, _) = json(
        &app,
        "GET",
        &format!("/api/v1/shell/fs?area=apps&path=&device={DEVICE}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Alice, without the assignment, keeps her own access.
    let (status, _) = json(
        &app,
        "GET",
        &format!("/api/v1/shell/fs?area=home&path=&device={DEVICE}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn cross_user_home_access_needs_admin_role_and_policy() {
    let temp = tempfile::tempdir().unwrap();
    let (app, alice, bob) = seeded(temp.path()).await;

    // Without the policy even the owner is refused.
    let resolve =
        format!("/api/v1/shell/fs/resolve?path=%2Fhome%2Fbob%2FDocuments&device={DEVICE}");
    let (status, _) = json(&app, "GET", &resolve, Some(&alice), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The policy is device-wide here: an administrator may now read bob's
    // home, a normal user may not, and the path identifies whose home it is.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.home.cross_user_read",
            "value": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, value) = json(&app, "GET", &resolve, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["path"], "/home/bob/Documents");
    assert_eq!(value["uri"], "rumahl://home/Documents");
    assert_eq!(value["writable"], false);
    assert!(value["id"].is_null());
    // A normal user never gets the cross-user path: their own home answers
    // with their name, alice's does not.
    let (own, _) = json(
        &app,
        "GET",
        &format!("/api/v1/shell/fs/resolve?path=%2Fhome%2Fbob&device={DEVICE}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(own, StatusCode::OK);
    let (status, _) = json(
        &app,
        "GET",
        &format!("/api/v1/shell/fs/resolve?path=%2Fhome%2Falice%2FDocuments&device={DEVICE}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Listing and downloading another home works through the user parameter.
    let list = format!("/api/v1/shell/fs?area=home&path=Documents&user=bob&device={DEVICE}");
    let (status, value) = json(&app, "GET", &list, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["entries"][0]["name"], "secret.txt");
    let content = format!(
        "/api/v1/shell/fs/content?area=home&path=Documents/secret.txt&user=bob&device={DEVICE}"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content)
                .header("cookie", format!("__Host-rumahl_session={alice}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"bob's secret");

    // A write that names another user is refused outright.
    let status = raw(&app, "PUT", &content, &alice, b"tampered".to_vec()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        std::fs::read(temp.path().join("home/bob/Documents/secret.txt")).unwrap(),
        b"bob's secret"
    );
}
