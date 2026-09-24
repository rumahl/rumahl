use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rumahl_core::AccountStateRepository;
use rumahl_persistence_sqlite::SqliteAccountStateRepository;
use rumahl_platform_service::*;
use rumahl_platform_web::{GatewayConfig, GatewayState, InMemoryShellEvents, router};
use std::{path::Path, sync::Arc};
use tower::ServiceExt;

const PASSWORD: &str = "a long integration test password";
fn app(root: &Path) -> Router {
    let accounts = root.join("accounts.sqlite");
    router(GatewayState {
        config: GatewayConfig::new("https://rumahl.test", root.join("ssr.sock")).unwrap(),
        backend: shell_backend(&accounts, &root.join("platform.sqlite"), "build-test", "de")
            .unwrap(),
        browser_sessions: Some(Arc::new(LocalBrowserSessions::open(&accounts).unwrap())),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: Arc::new(InMemoryShellEvents::new(8)),
        widgets: Arc::new(NoWidgets),
        streams: None,
        oidc: None,
    })
}
fn request(
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: &str,
    origin: Option<&str>,
) -> Request<Body> {
    let mut r = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        r = r.header("cookie", cookie);
    }
    if let Some(origin) = origin {
        r = r.header("origin", origin);
    }
    r.body(Body::from(body.to_owned())).unwrap()
}
async fn login(app: &Router, user: &str) -> String {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/login",
            None,
            &format!("username={user}&password={PASSWORD}"),
            Some("https://rumahl.test"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    for flag in [
        "Secure",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=43200",
    ] {
        assert!(cookie.contains(flag));
    }
    assert!(!cookie.contains("Domain="));
    cookie.split(';').next().unwrap().to_owned()
}
async fn snapshot(app: &Router, cookie: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/shell/snapshot",
            Some(cookie),
            "",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn two_users_survive_restart_and_logout_revokes_only_its_session() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("accounts.sqlite");
    for (name, display) in [("alice", "Alice"), ("bob", "Bob")] {
        provision(
            &db,
            name,
            display,
            PASSWORD.into(),
            LocalPasswordBlocklist::default(),
        )
        .unwrap();
    }
    let first = app(dir.path());
    let alice = login(&first, "alice").await;
    let bob = login(&first, "bob").await;
    assert_ne!(alice, bob);
    assert_eq!(
        snapshot(&first, &alice).await["user"]["displayName"],
        "Alice"
    );
    assert_eq!(snapshot(&first, &bob).await["user"]["displayName"], "Bob");
    drop(first);
    let restarted = app(dir.path());
    assert_eq!(
        snapshot(&restarted, &alice).await["user"]["displayName"],
        "Alice"
    );
    let response = restarted
        .clone()
        .oneshot(request(
            "POST",
            "/logout",
            Some(&alice),
            "",
            Some("https://rumahl.test"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    drop(restarted);
    let restarted = app(dir.path());
    let response = restarted
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/shell/snapshot",
            Some(&alice),
            "",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        snapshot(&restarted, &bob).await["user"]["displayName"],
        "Bob"
    );
    let state = SqliteAccountStateRepository::open(&db)
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(
        state
            .sessions()
            .sessions()
            .iter()
            .filter(|s| s.revoked_at().is_some())
            .count(),
        1
    );
}

#[tokio::test]
async fn csrf_malformed_forms_and_wrong_password_cannot_create_or_revoke_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("accounts.sqlite");
    provision(
        &db,
        "alice",
        "Alice",
        PASSWORD.into(),
        LocalPasswordBlocklist::default(),
    )
    .unwrap();
    let app = app(dir.path());
    let body = format!("username=alice&password={PASSWORD}");
    for origin in [None, Some("https://evil.test"), Some("null")] {
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/login", None, &body, origin))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    for body in [
        "username=alice&password=bad",
        "username=unknown&password=bad",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/login",
                    None,
                    body,
                    Some("https://rumahl.test")
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                "/login",
                None,
                &format!("{body}&password=duplicate"),
                Some("https://rumahl.test")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                "/login",
                None,
                &"x".repeat(8193),
                Some("https://rumahl.test")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(
        SqliteAccountStateRepository::open(&db)
            .unwrap()
            .load()
            .unwrap()
            .sessions()
            .is_empty()
    );
    let cookie = login(&app, "alice").await;
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                "/logout",
                Some(&cookie),
                "",
                Some("https://evil.test")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        snapshot(&app, &cookie).await["user"]["displayName"],
        "Alice"
    );
}

#[tokio::test]
async fn unauthenticated_navigation_reaches_login_even_when_renderer_is_down() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let response = app
        .clone()
        .oneshot(request("GET", "/", None, "", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/login");
    for path in ["/login", "/recovery"] {
        let response = app
            .clone()
            .oneshot(request("GET", path, None, "", None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
