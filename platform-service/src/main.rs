use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use rumahl_core::{AccountStateRepository, UnixTimestamp};
use rumahl_persistence_sqlite::SqliteAccountStateRepository;
use rumahl_platform_service::*;
use rumahl_platform_web::{GatewayConfig, GatewayState, InMemoryShellEvents, router, serve_router};
use rumahl_ui_contracts::ShellEvent;
use std::{
    collections::HashMap,
    env,
    io::{IsTerminal, Read},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use zeroize::Zeroizing;

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(16)
        .enable_all()
        .build()
        .expect("platform runtime could not start");
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("rumahl platform: {error}");
        std::process::exit(1);
    }
}

fn required(name: &str) -> Result<String, ServiceError> {
    env::var(name).map_err(|_| std::io::Error::other(format!("missing {name}")).into())
}
fn private_directory(path: &std::path::Path) -> Result<(), ServiceError> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0 {
        return Err(
            std::io::Error::other("state directory must already exist with mode 0700").into(),
        );
    }
    Ok(())
}
async fn run() -> Result<(), ServiceError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let state_dir = PathBuf::from(required("RUMAHL_STATE_DIR")?);
    if !state_dir.is_absolute() {
        return Err(std::io::Error::other("state directory must be absolute").into());
    }
    private_directory(&state_dir)?;
    let accounts = state_dir.join("accounts.sqlite");
    if args.first().map(String::as_str) == Some("provision-account") {
        if args.len() != 4 || args[3] != "--password-stdin" || std::io::stdin().is_terminal() {
            return Err(std::io::Error::other("usage: provision-account USER DISPLAY_NAME --password-stdin (pipe from a hidden prompt)").into());
        }
        let blocklist_path = required("RUMAHL_PASSWORD_BLOCKLIST")?;
        let blocklist = std::fs::read_to_string(blocklist_path)?;
        if blocklist.trim().is_empty() {
            return Err(std::io::Error::other("password blocklist is empty").into());
        }
        let mut password = Zeroizing::new(String::new());
        std::io::stdin().take(8193).read_to_string(&mut password)?;
        if password.len() > 8192 {
            return Err(std::io::Error::other("password input too large").into());
        }
        if password.ends_with('\n') {
            password.pop();
            if password.ends_with('\r') {
                password.pop();
            }
        }
        provision(
            &accounts,
            &args[1],
            &args[2],
            std::mem::take(&mut *password),
            LocalPasswordBlocklist(blocklist.lines().map(str::to_owned).collect()),
        )?;
        println!("Local account created.");
        return Ok(());
    }
    if args.as_slice() != ["serve"] {
        return Err(std::io::Error::other("usage: rumahl-platform-service serve").into());
    }
    let build_file = PathBuf::from(required("RUMAHL_CLIENT_BUILD")?);
    let build: serde_json::Value = serde_json::from_slice(&std::fs::read(build_file)?)?;
    let build_id = build
        .get("shellBuildId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| std::io::Error::other("client build ID missing"))?;
    let locale = env::var("RUMAHL_LOCALE").unwrap_or_else(|_| "en".into());
    let platform = state_dir.join("platform.sqlite");
    let config = GatewayConfig::new(
        &required("RUMAHL_PUBLIC_ORIGIN")?,
        required("RUMAHL_SSR_SOCKET")?,
    )
    .map_err(|_| std::io::Error::other("invalid gateway configuration"))?;
    let authority = config.public_origin.as_str()[8..].to_owned();
    let events = Arc::new(InMemoryShellEvents::new(128));
    let snapshots = PersistentShellSnapshots::open(&accounts, &platform, build_id, &locale)?;
    let account_feed = SqliteAccountStateRepository::open(&accounts)?;
    let state = GatewayState {
        config,
        backend: shell_backend(&accounts, &platform, build_id, &locale)?,
        browser_sessions: Some(Arc::new(LocalBrowserSessions::open(&accounts)?)),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: events.clone(),
        widgets: Arc::new(NoWidgets),
        streams: None,
        oidc: None,
    };
    let (theme_path, css) = stock_stylesheet();
    let app = router(state)
        .route(
            &theme_path,
            get(move || {
                let css = css.clone();
                async move {
                    let mut response = Response::new(Body::from(css));
                    response.headers_mut().insert(
                        "content-type",
                        HeaderValue::from_static("text/css; charset=utf-8"),
                    );
                    response.headers_mut().insert(
                        "cache-control",
                        HeaderValue::from_static("public, max-age=31536000, immutable"),
                    );
                    response.headers_mut().insert(
                        "x-content-type-options",
                        HeaderValue::from_static("nosniff"),
                    );
                    response
                }
            }),
        )
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let authority = authority.clone();
            async move {
                if request.headers().get_all("host").iter().count() != 1
                    || request.headers().get("host").and_then(|h| h.to_str().ok())
                        != Some(authority.trim_end_matches('/'))
                {
                    let mut response = Response::new(Body::empty());
                    *response.status_mut() = StatusCode::MISDIRECTED_REQUEST;
                    return response;
                }
                next.run(request).await
            }
        }));
    // Poll persisted data; the bus is only a notification layer. Reconnecting
    // clients always reload an authoritative snapshot after service restarts.
    let snapshots = Arc::new(snapshots);
    let account_feed = Arc::new(account_feed);
    tokio::spawn(async move {
        let mut revisions = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            let snapshots = snapshots.clone();
            let account_feed = account_feed.clone();
            let loaded =
                tokio::task::spawn_blocking(move || -> Result<HashMap<_, _>, ServiceError> {
                    let state = account_feed.load()?;
                    let now = UnixTimestamp::now()?;
                    let mut active = HashMap::new();
                    for session in state
                        .sessions()
                        .sessions()
                        .iter()
                        .filter(|s| s.is_active_at(now))
                    {
                        let user = session.user_id();
                        if active.contains_key(user) {
                            continue;
                        }
                        if let Ok(snapshot) = snapshots.for_user(user) {
                            active.insert(*user, snapshot.revision().to_owned());
                        }
                    }
                    Ok(active)
                })
                .await;
            if let Ok(Ok(active)) = loaded {
                for (user, revision) in &active {
                    if revisions.get(user).is_some_and(|old| old != revision)
                        && let Ok(event) = ShellEvent::snapshot_changed(revision)
                    {
                        events.publish(*user, event);
                    }
                }
                revisions = active;
            }
        }
    });
    let socket = PathBuf::from(required("RUMAHL_GATEWAY_SOCKET")?);
    let group = match env::var("RUMAHL_GATEWAY_SOCKET_ACCESS").as_deref() {
        Ok("group") => true,
        Ok("owner") | Err(_) => false,
        _ => return Err(std::io::Error::other("socket access must be owner or group").into()),
    };
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = serve_router(&socket, app, group) => result.map_err(|_| std::io::Error::other("gateway listener failed"))?,
        _ = terminate.recv() => {},
        _ = tokio::signal::ctrl_c() => {},
    }
    Ok(())
}
