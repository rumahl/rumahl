//! Router-level tests for the policy system: catalog, assignments, groups and
//! the three enforcement points (advanced mode, personal quota, cross-user
//! home access).
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rumahl_core::{AuditLog, InstallationId};
use rumahl_persistence_sqlite::SqliteAuditLog;
use rumahl_platform_service::*;
use rumahl_platform_web::{
    AppAccess, AppAccessError, AppAsset, AppData, AppSettingInfo, CatalogApp, GatewayConfig,
    GatewayState, ImportedApp, InMemoryShellEvents, PackageUploadFile, ShellIdentity, router,
};
use std::{path::Path, sync::Arc};
use tower::ServiceExt;

const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

fn password() -> String {
    rumahl_account_auth::SessionToken::generate()
        .unwrap()
        .encode()
        .to_string()
}

/// The shared gateway state: administration, audit, policies, files.
fn state_of(
    root: &Path,
    admin: Arc<LocalAdministration>,
    audit: Arc<dyn AuditLog>,
) -> GatewayState {
    let accounts = root.join("accounts.sqlite");
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
    GatewayState {
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
        browser_sessions: Some(Arc::new(
            LocalBrowserSessions::open(&accounts)
                .unwrap()
                .with_policies(policies.clone()),
        )),
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
    }
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
    router(state_of(root, admin, audit))
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
            "rumahl.password.min_length",
            "rumahl.password.require_classes",
            "rumahl.password.attempt_limit",
            "rumahl.password.lockout_seconds",
            "rumahl.session.lifetime_seconds",
            "rumahl.os.advanced_mode",
            "rumahl.os.allow_store_install",
            "rumahl.os.allow_app_uninstall",
            "rumahl.home.cross_user_read",
            "rumahl.files.quota_bytes",
            "rumahl.files.max_file_bytes",
            "rumahl.files.durable_grants",
            "rumahl.apps.storage_quota_bytes",
            "rumahl.apps.allowed_publishers"
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

/// A login that returns the whole response, for asserting cookie attributes.
async fn login_raw(app: &Router, username: &str, password: &str) -> axum::http::Response<Body> {
    app.clone()
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
        .unwrap()
}

#[tokio::test]
async fn password_rules_reach_account_creation() {
    let temp = tempfile::tempdir().unwrap();
    let (app, alice, _bob) = seeded(temp.path()).await;

    // Raise the bar: 24 characters from all four classes.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.password.min_length",
            "value": 24
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.password.require_classes",
            "value": 4
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // A short one-class password is refused; the account never appears.
    let (status, _) = json(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&alice),
        Some(serde_json::json!({
            "username": "carol", "displayName": "Carol",
            "password": "onlylowercase", "role": "user"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, users) = json(&app, "GET", "/api/v1/admin/users", Some(&alice), None).await;
    assert!(
        users["users"]
            .as_array()
            .unwrap()
            .iter()
            .all(|user| user["username"] != "carol")
    );

    // A compliant password creates the account.
    let (status, _) = json(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&alice),
        Some(serde_json::json!({
            "username": "carol", "displayName": "Carol",
            "password": "Four-Class-Password_2026!", "role": "user"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Lowering the rule back lets simpler passwords in again.
    let (status, _) = json(
        &app,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.password.require_classes",
            "value": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = json(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&alice),
        Some(serde_json::json!({
            "username": "dave", "displayName": "Dave",
            "password": "lowercase-but-long-enough", "role": "user"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_session_lifetime_follows_the_policy() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let accounts = root.join("accounts.sqlite");
    let alice_password = password();
    provision(
        &accounts,
        "alice",
        "Alice",
        alice_password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let router = app(root);

    // The default lifetime is 12 hours.
    let response = login_raw(&router, "alice", &alice_password).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    assert!(cookie.contains("Max-Age=43200"), "{cookie}");

    let alice = login(&router, "alice", &alice_password).await;
    let (status, _) = json(
        &router,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.session.lifetime_seconds",
            "value": 600
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // A policy shortens it to 10 minutes; the cookie follows the very next
    // login, without a restart.
    let response = login_raw(&router, "alice", &alice_password).await;
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    assert!(cookie.contains("Max-Age=600"), "{cookie}");
}

/// One fixed installation so route-level tests can address it.
fn gate_installation() -> InstallationId {
    InstallationId::parse("01990000-0000-7000-8000-0000000000aa").unwrap()
}

/// A minimal provider so the import/uninstall gates can be exercised without
/// a signed package or a runtime: one web app, one fixed installation.
struct GateApps;

impl rumahl_platform_web::AppProvider for GateApps {
    fn catalog(&self, _identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
        Ok(vec![CatalogApp {
            id: "com.rumahl.gate".into(),
            installation_id: gate_installation(),
            title: "Gate".into(),
            version: "1.0.0".into(),
            launchable: true,
            capabilities: Vec::new(),
            runtime: "web".into(),
            lifecycle: "on-demand".into(),
            connectors: Vec::new(),
        }])
    }
    fn entrypoint(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<String, AppAccessError> {
        Ok("/index.html".into())
    }
    fn asset(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _path: &str,
    ) -> Result<AppAsset, AppAccessError> {
        Err(AppAccessError::Denied)
    }
    fn settings(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<Vec<AppSettingInfo>, AppAccessError> {
        Ok(Vec::new())
    }
    fn data(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
        _path: &str,
    ) -> Result<AppData, AppAccessError> {
        Err(AppAccessError::Denied)
    }
    fn import_package(
        &self,
        _identity: ShellIdentity,
        _files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        Ok(ImportedApp {
            id: "com.rumahl.gate".into(),
            installation_id: gate_installation(),
            title: "Gate".into(),
            version: "1.0.0".into(),
        })
    }
    fn uninstall_app(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<(), AppAccessError> {
        Ok(())
    }
}

/// The policy-test gateway, this time with a minimal app catalog.
fn app_with_apps(root: &Path) -> Router {
    let accounts = root.join("accounts.sqlite");
    let audit: Arc<dyn AuditLog> =
        Arc::new(SqliteAuditLog::open(root.join("audit.sqlite")).unwrap());
    let admin = Arc::new(
        LocalAdministration::open(&accounts, Some(audit.clone()))
            .unwrap()
            .with_home_root(root.join("home")),
    );
    let mut state = state_of(root, admin, audit);
    let policies = state.policies.clone().unwrap();
    let origin = GatewayConfig::new("https://rumahl.test", root.join("ssr.sock"))
        .unwrap()
        .public_origin;
    state.apps = Some(Arc::new(
        AppAccess::new(Arc::new(GateApps), &origin, "apps.rumahl.test")
            .unwrap()
            .with_policies(policies),
    ));
    router(state)
}

#[tokio::test]
async fn installs_and_uninstalls_can_be_reserved_for_administrators() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let accounts = root.join("accounts.sqlite");
    let alice_password = password();
    provision(
        &accounts,
        "alice",
        "Alice",
        alice_password.clone(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let router = app_with_apps(root);
    let alice = login(&router, "alice", &alice_password).await;

    // A second, normal account to be refused by the gates.
    let (status, _) = json(
        &router,
        "POST",
        "/api/v1/admin/users",
        Some(&alice),
        Some(serde_json::json!({
            "username": "bob", "displayName": "Bob",
            "password": "a-long-enough-development-password", "role": "user"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let bob = login(&router, "bob", "a-long-enough-development-password").await;

    let (status, _) = json(
        &router,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.os.allow_store_install",
            "value": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let body = serde_json::json!({
        "importVersion": 1,
        "device": "00000000-0000-4000-8000-000000000001",
        "installOrigin": "store",
        "files": [
            {"path": "package/manifest.json", "content": "e30="}
        ]
    });
    let import = |cookie: &str| {
        router.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/shell/apps/import")
                .header("content-type", "application/json")
                .header("origin", "https://rumahl.test")
                .header("cookie", format!("__Host-rumahl_session={cookie}"))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
    };
    // A normal user is refused by the policy; the owner still imports.
    let response = import(&bob).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = import(&alice).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // The uninstall gate follows the same pattern.
    let (status, _) = json(
        &router,
        "PUT",
        "/api/v1/admin/policies",
        Some(&alice),
        Some(serde_json::json!({
            "subject": "device",
            "key": "rumahl.os.allow_app_uninstall",
            "value": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let uninstall = |cookie: &str| {
        router.clone().oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/shell/apps/com.rumahl.gate")
                .header("content-type", "application/json")
                .header("origin", "https://rumahl.test")
                .header("cookie", format!("__Host-rumahl_session={cookie}"))
                .body(Body::from(
                    serde_json::json!({"installationId": gate_installation().to_string()})
                        .to_string(),
                ))
                .unwrap(),
        )
    };
    let response = uninstall(&bob).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = uninstall(&alice).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Turning the policies back returns both rights to everyone.
    for key in [
        "rumahl.os.allow_store_install",
        "rumahl.os.allow_app_uninstall",
    ] {
        let (status, _) = json(
            &router,
            "PUT",
            "/api/v1/admin/policies",
            Some(&alice),
            Some(serde_json::json!({"subject": "device", "key": key, "value": null})),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let response = import(&bob).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = uninstall(&bob).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
