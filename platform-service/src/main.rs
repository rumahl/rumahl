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
use rumahl_ui_contracts::{ShellEvent, SystemState};
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

/// Device addresses published by rumahl-hosts.service. The platform sandbox
/// blocks netlink, so interface enumeration is only a fallback for hosts that
/// do not run that service.
fn configured_device_ips() -> Vec<std::net::IpAddr> {
    let path = env::var("RUMAHL_DEVICE_IPS_FILE").unwrap_or_else(|_| "/run/rumahl/ips".to_owned());
    match std::fs::read_to_string(path) {
        Ok(value) => value
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| line.parse().ok())
            .collect(),
        Err(_) => local_unicast_addresses(),
    }
}

/// Current non-loopback unicast IPv4/IPv6 addresses of this device.
fn local_unicast_addresses() -> Vec<std::net::IpAddr> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    let mut addresses = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // Safety: getifaddrs fills a NULL-terminated list owned by libc, which
    // freeifaddrs releases exactly once below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return addresses;
    }
    let mut current = list;
    while !current.is_null() {
        let entry = unsafe { &*current };
        let address = entry.ifa_addr;
        if !address.is_null() {
            match unsafe { (*address).sa_family } as i32 {
                libc::AF_INET => {
                    let raw = unsafe { &*(address as *const libc::sockaddr_in) };
                    let ip = Ipv4Addr::from(u32::from_be(raw.sin_addr.s_addr));
                    if !ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local() {
                        addresses.push(IpAddr::V4(ip));
                    }
                }
                libc::AF_INET6 => {
                    let raw = unsafe { &*(address as *const libc::sockaddr_in6) };
                    let ip = Ipv6Addr::from(raw.sin6_addr.s6_addr);
                    let link_local = ip
                        .segments()
                        .first()
                        .is_some_and(|segment| segment & 0xffc0 == 0xfe80);
                    if !ip.is_loopback()
                        && !ip.is_unspecified()
                        && !link_local
                        && ip.to_ipv4_mapped().is_none()
                    {
                        addresses.push(IpAddr::V6(ip));
                    }
                }
                _ => {}
            }
        }
        current = entry.ifa_next;
    }
    unsafe { libc::freeifaddrs(list) };
    addresses.sort();
    addresses.dedup();
    addresses
}

/// Builds the field-encryption key provider from the TPM environment, if set.
/// A deployment without TPM stores file content as plaintext.
fn optional_key_provider() -> Option<Arc<dyn rumahl_persistence_sqlite::FieldKeyProvider>> {
    let executable = env::var("RUMAHL_TPM2_EXECUTABLE").ok()?;
    let key_id = rumahl_persistence_sqlite::SecretEncryptionKeyId::parse(
        env::var("RUMAHL_SECRET_KEY_ID").ok()?,
    )
    .ok()?;
    let object =
        rumahl_platform_buildroot::Tpm2Object::parse(&env::var("RUMAHL_SECRET_KEY_OBJECT").ok()?)
            .ok()?;
    let authorization = match env::var("RUMAHL_SECRET_KEY_POLICY_SESSION") {
        Ok(value) if !value.is_empty() => {
            rumahl_platform_buildroot::Tpm2Authorization::PolicySession(PathBuf::from(value))
        }
        _ => rumahl_platform_buildroot::Tpm2Authorization::Passwordless,
    };
    let sealed =
        rumahl_platform_buildroot::Tpm2SealedKey::new(key_id.clone(), object, authorization)
            .ok()?;
    rumahl_platform_buildroot::Tpm2UnsealKeyProvider::new(executable, key_id, [sealed])
        .ok()
        .map(|provider| Arc::new(provider) as Arc<dyn rumahl_persistence_sqlite::FieldKeyProvider>)
}

async fn run() -> Result<(), ServiceError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let state_dir = PathBuf::from(required("RUMAHL_STATE_DIR")?);
    if !state_dir.is_absolute() {
        return Err(std::io::Error::other("state directory must be absolute").into());
    }
    private_directory(&state_dir)?;
    // A stale stop-intent from a previous run must never label a fresh start.
    let _ = std::fs::remove_file(state_dir.join("lifecycle"));
    let accounts = state_dir.join("accounts.sqlite");
    // Refuse a database written by a newer binary before any provider opens it.
    for database in [
        accounts.clone(),
        state_dir.join("platform.sqlite"),
        state_dir.join("preferences.sqlite"),
        state_dir.join("files.sqlite"),
    ] {
        rumahl_persistence_sqlite::verify_schema(&database).map_err(|error| {
            std::io::Error::other(format!("database schema check failed: {error}"))
        })?;
    }
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
    if args.first().map(String::as_str) == Some("install-package") {
        if args.len() != 2 {
            return Err(std::io::Error::other(
                "usage: install-package PACKAGE_DIR (see install module)",
            )
            .into());
        }
        install::install_package(&state_dir, std::path::Path::new(&args[1]))?;
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("backup") {
        if args.len() != 2 {
            return Err(std::io::Error::other("usage: backup DESTINATION_DIR").into());
        }
        backup_state(&state_dir, std::path::Path::new(&args[1]))?;
        println!("State backup written.");
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
    let primary_origin = required("RUMAHL_PUBLIC_ORIGIN")?;
    let mut config = GatewayConfig::new(&primary_origin, required("RUMAHL_SSR_SOCKET")?)
        .map_err(|_| std::io::Error::other("invalid gateway configuration"))?;
    let primary_host = config
        .public_origin
        .host_str()
        .unwrap_or_default()
        .to_owned();
    let port_suffix = config
        .public_origin
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    // RUMAHL_PUBLIC_HOSTS is a comma-separated allowlist of shell host names.
    // The token IP expands to the device's current unicast addresses, which are
    // always accepted so a device stays reachable when local name resolution
    // is unavailable.
    let mut host_names: Vec<String> = env::var("RUMAHL_PUBLIC_HOSTS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty() && !entry.eq_ignore_ascii_case("ip"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if host_names.is_empty() {
        host_names.push(primary_host.clone());
    }
    let device_ips = configured_device_ips();
    let mut allowed_origins: Vec<String> = host_names
        .iter()
        .map(|host| format!("https://{host}{port_suffix}"))
        .collect();
    allowed_origins.extend(device_ips.iter().map(|ip| match ip {
        std::net::IpAddr::V6(_) => format!("https://[{ip}]{port_suffix}"),
        std::net::IpAddr::V4(_) => format!("https://{ip}{port_suffix}"),
    }));
    config
        .add_allowed_origins(allowed_origins)
        .map_err(|_| std::io::Error::other("invalid public host configuration"))?;
    let allowed_authorities: Vec<String> = config
        .allowed_origins
        .iter()
        .map(|origin| origin.as_str()[8..].trim_end_matches('/').to_owned())
        .collect();
    // App hosts use per-host suffixes. RUMAHL_APP_HOST_SUFFIX keeps a single
    // explicit suffix for the primary host for backwards compatibility.
    let mut app_suffixes: Vec<String> = Vec::new();
    if let Ok(explicit) = env::var("RUMAHL_APP_HOST_SUFFIX")
        && !explicit.is_empty()
    {
        app_suffixes.push(explicit);
    }
    for host in &host_names {
        let suffix = format!("apps.{host}");
        if !app_suffixes.contains(&suffix) {
            app_suffixes.push(suffix);
        }
    }
    let app_root = state_dir.join("app-assets");
    let apps_provider = apps::PersistentApps::open(&accounts, &platform, app_root)?;
    let apps_provider = match apps_data_root() {
        Some(data_root) => apps_provider.with_data_root(data_root)?,
        None => apps_provider,
    };
    // Runtime status/start/stop through the supervisor when it is configured;
    // otherwise the runtime API stays fail-closed (503). The same channel
    // registry receives live app channels for background event delivery.
    let runtime_channels = Arc::new(rumahl_core::RuntimeChannelRegistry::new());
    let apps_provider = match install::runtime_adapters_from_env(&runtime_channels) {
        Some(adapters) => apps_provider.with_runtime_adapters(adapters),
        None => apps_provider,
    };
    let app_ports: Option<Arc<dyn rumahl_platform_web::AppPortProvider>> = match (
        env::var("RUMAHL_HOST_CONFIG_SOCKET"),
        env::var("RUMAHL_APP_PORTS_FILE"),
    ) {
        (Ok(socket), Ok(ports_file)) => {
            let socket = PathBuf::from(socket);
            let ports_file = PathBuf::from(ports_file);
            if socket.is_absolute() && ports_file.is_absolute() {
                Some(Arc::new(
                    rumahl_platform_service::app_ports::BrokerAppPorts::new(socket, ports_file),
                ))
            } else {
                None
            }
        }
        _ => None,
    };
    let apps = Arc::new(
        rumahl_platform_web::AppAccess::new(
            Arc::new(apps_provider),
            &config.public_origin,
            &config.allowed_origins,
            &app_suffixes,
            app_ports,
        )
        .map_err(|_| std::io::Error::other("invalid app host configuration"))?,
    );
    let events = Arc::new(InMemoryShellEvents::new(128));
    // Kept for the shutdown signal: the OS tells every client when it stops.
    let lifecycle_events = events.clone();
    let snapshot_store = Arc::new(rumahl_persistence_sqlite::SqliteShellPreferences::open(
        state_dir.join("preferences.sqlite"),
    )?);
    let snapshots = PersistentShellSnapshots::open(&accounts, &platform, build_id, &locale)?
        .with_preferences(snapshot_store.clone())
        .with_workspace(snapshot_store.clone());
    let account_feed = SqliteAccountStateRepository::open(&accounts)?;
    let app_store = Arc::new(rumahl_platform_service::store::AppStore::open(&state_dir)?);
    let audit: Arc<dyn rumahl_core::AuditLog> = Arc::new(
        rumahl_persistence_sqlite::SqliteAuditLog::open(state_dir.join("audit.sqlite"))?,
    );
    record_os_event(
        audit.as_ref(),
        rumahl_core::AuditAction::SystemStarted,
        rumahl_core::AuditOutcome::Success,
        Some("platform"),
    );
    tokio::spawn(ingest_os_events(
        audit.clone(),
        state_dir.join("os-events.jsonl"),
    ));
    tokio::spawn(monitor_sensors(audit.clone()));
    let state = GatewayState {
        config,
        backend: shell_backend(&accounts, &platform, build_id, &locale)?,
        browser_sessions: Some(Arc::new(
            LocalBrowserSessions::open(&accounts)?.with_audit(audit.clone()),
        )),
        login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        events: events.clone(),
        widgets: Arc::new(NoWidgets),
        streams: None,
        apps: Some(apps.clone()),
        store: Some(app_store),
        files: Some(match optional_key_provider() {
            Some(provider) => Arc::new(
                rumahl_persistence_sqlite::SqlitePersonalFiles::open_encrypted(
                    state_dir.join("files.sqlite"),
                    provider,
                )?,
            ),
            None => {
                eprintln!(
                    "rumahl platform: file content encryption disabled (no TPM key configured)"
                );
                Arc::new(rumahl_persistence_sqlite::SqlitePersonalFiles::open(
                    state_dir.join("files.sqlite"),
                )?)
            }
        }),
        workspace: Some(Arc::new(
            rumahl_persistence_sqlite::SqliteShellPreferences::open(
                state_dir.join("preferences.sqlite"),
            )?,
        )),
        preferences: Some(Arc::new(
            rumahl_persistence_sqlite::SqliteShellPreferences::open(
                state_dir.join("preferences.sqlite"),
            )?,
        )),
        os_mode: Some(Arc::new(AuditedOsModeRepository::new(
            Arc::new(rumahl_persistence_sqlite::SqliteOsModeRepository::open(
                state_dir.join("preferences.sqlite"),
            )?),
            audit.clone(),
        ))),
        host_files: Some(Arc::new(
            rumahl_platform_service::host_files::LocalHostFiles::from_environment(),
        )),
        oidc: None,
        audit: Some(audit.clone()),
        state_dir: Some(state_dir.clone()),
    };
    // Serve every built-in theme stylesheet so the account theme renders on the
    // first paint without a flash of the default theme.
    let mut app = router(state);
    for (path, css) in theme_stylesheets() {
        app = app.route(
            &path,
            get(move || {
                let css = css.clone();
                async move { stylesheet_response(css) }
            }),
        );
    }
    let app = app.layer(middleware::from_fn(move |request: Request, next: Next| {
        let allowed_authorities = allowed_authorities.clone();
        let apps = apps.clone();
        async move {
            let host = request
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_owned();
            if request.headers().get_all("host").iter().count() != 1 {
                return rumahl_platform_web::app_error(rumahl_platform_web::AppAccessError::Denied);
            }
            if apps.accepts_host(&host) {
                if request.method() != axum::http::Method::GET
                    && request.method() != axum::http::Method::HEAD
                {
                    return rumahl_platform_web::app_error(
                        rumahl_platform_web::AppAccessError::Denied,
                    );
                }
                let path = request.uri().path().to_owned();
                let head = request.method() == axum::http::Method::HEAD;
                let loader = apps.clone();
                let load_host = host.clone();
                return match tokio::task::spawn_blocking(move || loader.resource(&load_host, &path))
                    .await
                {
                    Ok(Ok(asset)) => {
                        let mut response = apps.asset_response(&host, asset);
                        if head {
                            *response.body_mut() = Body::empty();
                        }
                        response
                    }
                    Ok(Err(error)) => rumahl_platform_web::app_error(error),
                    Err(_) => rumahl_platform_web::app_error(
                        rumahl_platform_web::AppAccessError::Unavailable,
                    ),
                };
            }
            if !allowed_authorities.iter().any(|allowed| allowed == &host) {
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
                        if let Ok(snapshot) = snapshots.for_user(user, None) {
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
    // Tell every connected client that the whole system is stopping (restart or
    // power-off). Only the server knows this; clients never infer it from time.
    lifecycle_events.publish_all(ShellEvent::system_lifecycle(lifecycle_intent(&state_dir)));
    // Give the WebSocket fan-out a moment to flush before the process exits.
    tokio::time::sleep(Duration::from_millis(800)).await;
    record_os_event(
        audit.as_ref(),
        rumahl_core::AuditAction::SystemStopped,
        rumahl_core::AuditOutcome::Success,
        Some("platform"),
    );
    Ok(())
}

/// Records an operating-system level event (boot, services, supervisor,
/// packages). These are always attributed to "rumahl OS" for the user.
fn record_os_event(
    audit: &dyn rumahl_core::AuditLog,
    action: rumahl_core::AuditAction,
    outcome: rumahl_core::AuditOutcome,
    target: Option<&str>,
) {
    let at = rumahl_core::UnixTimestamp::now()
        .unwrap_or_else(|_| rumahl_core::UnixTimestamp::from_seconds(0));
    let base = || rumahl_core::NewAuditEvent::new(at, rumahl_core::AuditActor::Os, action, outcome);
    let event = match target {
        Some(target) => base()
            .with_target(target.to_owned())
            .unwrap_or_else(|_| base()),
        None => base(),
    };
    let _ = audit.record(event);
}

/// Reads the stop intent the host broker writes before stopping the platform
/// (`<state>/lifecycle`, either `restarting` or `shutting-down`). A missing or
/// stale file means a restart, which is the common case for a service stop.
fn lifecycle_intent(state_dir: &std::path::Path) -> SystemState {
    let path = state_dir.join("lifecycle");
    let fresh = std::fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 300);
    if fresh
        && let Ok(text) = std::fs::read_to_string(&path)
        && text.trim() == "shutting-down"
    {
        return SystemState::ShuttingDown;
    }
    SystemState::Restarting
}

/// Ingests the OS/Buildroot event spool (`os-events.jsonl`): every line the OS
/// layer reports becomes a central audit event, attributed to "rumahl OS".
async fn ingest_os_events(audit: Arc<dyn rumahl_core::AuditLog>, spool: PathBuf) {
    let mut processed = 0usize;
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        let Ok(text) = std::fs::read_to_string(&spool) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() <= processed {
            continue;
        }
        for line in &lines[processed..] {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let action = value
                .get("action")
                .and_then(|v| v.as_str())
                .and_then(|s| rumahl_core::AuditAction::parse(s).ok())
                .unwrap_or(rumahl_core::AuditAction::Activity);
            let outcome = value
                .get("outcome")
                .and_then(|v| v.as_str())
                .and_then(|s| rumahl_core::AuditOutcome::parse(s).ok())
                .unwrap_or(rumahl_core::AuditOutcome::Success);
            let target = value.get("target").and_then(|v| v.as_str());
            record_os_event(audit.as_ref(), action, outcome, target);
        }
        processed = lines.len();
    }
}

/// Watches host sensors and records OS events when the level changes, so the
/// activity log and the recovery hub see resource pressure as it happens.
async fn monitor_sensors(audit: Arc<dyn rumahl_core::AuditLog>) {
    let mut last = "information";
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    loop {
        interval.tick().await;
        let level = rumahl_platform_web::read_sensors().level;
        if level == last {
            continue;
        }
        let (action, outcome, target) = match level {
            "error" => (
                rumahl_core::AuditAction::SelfHeal,
                rumahl_core::AuditOutcome::Failure,
                "sensor critical",
            ),
            "warning" => (
                rumahl_core::AuditAction::Maintenance,
                rumahl_core::AuditOutcome::Success,
                "sensor warning",
            ),
            _ => (
                rumahl_core::AuditAction::ServiceRecovered,
                rumahl_core::AuditOutcome::Success,
                "sensor recovered",
            ),
        };
        record_os_event(audit.as_ref(), action, outcome, Some(target));
        last = level;
    }
}

/// Immutable, content-addressed stylesheet response for a built-in theme.
fn stylesheet_response(css: String) -> Response {
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
