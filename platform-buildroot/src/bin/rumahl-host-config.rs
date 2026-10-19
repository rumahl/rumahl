//! Privileged host configuration broker.
//!
//! Runs as root and owns three device-lifecycle operations that the unprivileged
//! platform service must not perform:
//!
//! - allocate a dynamic app port and write the matching nginx server block,
//! - release it again,
//! - re-sign the device certificate with the current host names and IP SANs.
//!
//! Requests are authenticated with SO_PEERCRED and accept only an installation
//! ID. Paths and commands are fixed in configuration and never come from the
//! client.
use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_REQUEST: usize = 8192;

struct Config {
    socket: PathBuf,
    state_dir: PathBuf,
    app_conf_dir: PathBuf,
    ports_file: PathBuf,
    tls_dir: PathBuf,
    ca_dir: PathBuf,
    hosts_file: PathBuf,
    ips_file: PathBuf,
    platform_uid: u32,
    socket_gid: Option<u32>,
}

#[derive(Default)]
struct State {
    ports: BTreeMap<String, u16>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rumahl host config failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args(std::env::args_os().skip(1))?;
    std::fs::create_dir_all(&config.state_dir)?;
    std::fs::create_dir_all(&config.app_conf_dir)?;
    let state = Arc::new(Mutex::new(load_state(&config)?));
    {
        let state = state.lock().map_err(|_| "state lock poisoned")?;
        write_all_app_confs(&config, &state)?;
        write_ports_file(&config, &state)?;
    }
    if let Err(error) = refresh_tls(&config) {
        eprintln!("host config: TLS refresh skipped: {error}");
    }
    let _ = reload_nginx(&config);

    if let Some(parent) = config.socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&config.socket);
    let listener = UnixListener::bind(&config.socket)?;
    configure_socket(&config)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = serve(&config, &state, stream) {
                    eprintln!("host config: request rejected: {error}");
                }
            }
            Err(error) => eprintln!("host config: accept failed: {error}"),
        }
    }
    Ok(())
}

fn serve(
    config: &Config,
    state: &Arc<Mutex<State>>,
    mut stream: UnixStream,
) -> Result<(), Box<dyn std::error::Error>> {
    if peer_uid(&stream)? != config.platform_uid {
        return Err("peer UID mismatch".into());
    }
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > MAX_REQUEST {
            return Err("request too large".into());
        }
        if chunk[..read].contains(&b'\n') {
            break;
        }
    }
    let text = std::str::from_utf8(&request)?;
    let line = text.lines().next().unwrap_or("");
    let value: serde_json::Value = serde_json::from_str(line)?;
    let operation = value
        .get("op")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let response = match operation {
        "ports" => {
            let state = state.lock().map_err(|_| "state lock poisoned")?;
            serde_json::json!({ "ok": true, "ports": state.ports })
        }
        "allocate" => {
            let installation = installation_arg(&value)?;
            let mut state = state.lock().map_err(|_| "state lock poisoned")?;
            match allocate(config, &mut state, &installation) {
                Ok(port) => serde_json::json!({ "ok": true, "port": port }),
                Err(error) => serde_json::json!({ "ok": false, "error": error.to_string() }),
            }
        }
        "release" => {
            let installation = installation_arg(&value)?;
            let mut state = state.lock().map_err(|_| "state lock poisoned")?;
            match release(config, &mut state, &installation) {
                Ok(()) => serde_json::json!({ "ok": true }),
                Err(error) => serde_json::json!({ "ok": false, "error": error.to_string() }),
            }
        }
        "refresh-tls" => match refresh_tls(config) {
            Ok(()) => {
                let _ = reload_nginx(config);
                serde_json::json!({ "ok": true })
            }
            Err(error) => serde_json::json!({ "ok": false, "error": error.to_string() }),
        },
        _ => serde_json::json!({ "ok": false, "error": "unknown operation" }),
    };
    serde_json::to_writer(&mut stream, &response)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn installation_arg(value: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    let installation = value
        .get("installation")
        .and_then(|value| value.as_str())
        .ok_or("missing installation")?;
    if !valid_installation(installation) {
        return Err("invalid installation".into());
    }
    Ok(installation.to_owned())
}

fn valid_installation(value: &str) -> bool {
    (32..=36).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || byte == b'-')
}

fn allocate(config: &Config, state: &mut State, installation: &str) -> io::Result<u16> {
    if let Some(port) = state.ports.get(installation) {
        return Ok(*port);
    }
    // The kernel picks a free port. Drop the listener before nginx reloads, so
    // nginx can bind it; the window is short and a collision simply retries.
    let listener = std::net::TcpListener::bind(("0.0.0.0", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    state.ports.insert(installation.to_owned(), port);
    write_app_conf(config, port)?;
    save_state(config, state)?;
    write_ports_file(config, state)?;
    let _ = reload_nginx(config);
    Ok(port)
}

fn release(config: &Config, state: &mut State, installation: &str) -> io::Result<()> {
    if let Some(port) = state.ports.remove(installation) {
        let _ = std::fs::remove_file(config.app_conf_dir.join(format!("{port}.conf")));
        save_state(config, state)?;
        write_ports_file(config, state)?;
        let _ = reload_nginx(config);
    }
    Ok(())
}

fn write_all_app_confs(config: &Config, state: &State) -> io::Result<()> {
    if let Ok(entries) = std::fs::read_dir(&config.app_conf_dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .is_some_and(|value| value == "conf")
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    for port in state.ports.values() {
        write_app_conf(config, *port)?;
    }
    Ok(())
}

fn write_app_conf(config: &Config, port: u16) -> io::Result<()> {
    std::fs::create_dir_all(&config.app_conf_dir)?;
    let body = format!(
        r#"server {{
    listen {port} ssl;
    server_name _;
    ssl_certificate {tls}/fullchain.pem;
    ssl_certificate_key {tls}/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    client_max_body_size 1k;
    access_log off;
    add_header Strict-Transport-Security "max-age=31536000" always;
    add_header X-Content-Type-Options nosniff always;
    location / {{
        proxy_pass http://rumahl_platform;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header Origin $http_origin;
        proxy_set_header Cookie "";
        proxy_set_header Authorization "";
        proxy_set_header Connection close;
        proxy_set_header X-User-Id "";
        proxy_set_header X-Session-Id "";
        proxy_set_header X-Forwarded-User "";
        proxy_buffering off;
        proxy_read_timeout 15s;
    }}
}}
"#,
        tls = config.tls_dir.display()
    );
    atomic_write(
        &config.app_conf_dir.join(format!("{port}.conf")),
        body.as_bytes(),
        0o600,
    )
}

fn load_state(config: &Config) -> io::Result<State> {
    let path = config.state_dir.join("app-ports.json");
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(State::default());
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let mut state = State::default();
    if let Some(ports) = value.get("ports").and_then(|value| value.as_object()) {
        for (installation, port) in ports {
            if !valid_installation(installation) {
                continue;
            }
            if let Some(port) = port.as_u64().and_then(|port| u16::try_from(port).ok())
                && port > 1023
            {
                state.ports.insert(installation.clone(), port);
            }
        }
    }
    Ok(state)
}

fn save_state(config: &Config, state: &State) -> io::Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({ "ports": &state.ports }))?;
    atomic_write(&config.state_dir.join("app-ports.json"), &body, 0o600)
}

/// Read-only copy for the unprivileged platform service.
fn write_ports_file(config: &Config, state: &State) -> io::Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({ "ports": &state.ports }))?;
    atomic_write(&config.ports_file, &body, 0o644)
}

fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(io::Error::other("path has no parent"));
    };
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("new");
    {
        let mut file = std::fs::File::create(&temporary)?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)?;
    if let Ok(directory) = std::fs::File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn refresh_tls(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let ca_cert = config.ca_dir.join("ca.pem");
    let ca_key = config.ca_dir.join("ca-key.pem");
    if !ca_cert.is_file() || !ca_key.is_file() {
        return Err("device CA is not provisioned".into());
    }
    let hosts = read_lines(&config.hosts_file);
    let ips = read_lines(&config.ips_file);
    if hosts.is_empty() {
        return Err("no shell hosts are configured".into());
    }
    let mut sans = Vec::new();
    for host in &hosts {
        sans.push(format!("DNS:{host}"));
        sans.push(format!("DNS:*.apps.{host}"));
    }
    for ip in &ips {
        if ip.parse::<std::net::IpAddr>().is_ok() {
            sans.push(format!("IP:{ip}"));
        }
    }
    let fingerprint = sans.join(",");
    let sans_file = config.tls_dir.join(".sans");
    let cert = config.tls_dir.join("fullchain.pem");
    let key = config.tls_dir.join("key.pem");
    if cert.is_file()
        && key.is_file()
        && std::fs::read_to_string(&sans_file).ok().as_deref() == Some(fingerprint.as_str())
    {
        return Ok(());
    }

    let work = config.state_dir.join("tls-work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700))?;

    let server_conf = format!(
        "[req]\ndistinguished_name = dn\nreq_extensions = req_ext\nprompt = no\n[dn]\nCN = {primary}\n[req_ext]\nsubjectAltName = {sans}\n",
        primary = hosts[0],
        sans = sans.join(",")
    );
    std::fs::write(work.join("server.cnf"), server_conf)?;
    let ext_conf = format!(
        "subjectAltName = {sans}\nbasicConstraints = CA:FALSE\nkeyUsage = digitalSignature, keyEncipherment\nextendedKeyUsage = serverAuth\n",
        sans = sans.join(", ")
    );
    std::fs::write(work.join("ext.cnf"), ext_conf)?;

    let openssl = "/usr/bin/openssl";
    run_command(
        openssl,
        &[
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            work.join("key.pem").to_str().ok_or("non-UTF-8 path")?,
            "-out",
            work.join("server.csr").to_str().ok_or("non-UTF-8 path")?,
            "-config",
            work.join("server.cnf").to_str().ok_or("non-UTF-8 path")?,
        ],
    )?;
    run_command(
        openssl,
        &[
            "x509",
            "-req",
            "-in",
            work.join("server.csr").to_str().ok_or("non-UTF-8 path")?,
            "-CA",
            ca_cert.to_str().ok_or("non-UTF-8 path")?,
            "-CAkey",
            ca_key.to_str().ok_or("non-UTF-8 path")?,
            "-CAserial",
            work.join("ca.srl").to_str().ok_or("non-UTF-8 path")?,
            "-CAcreateserial",
            "-out",
            work.join("server.crt").to_str().ok_or("non-UTF-8 path")?,
            "-days",
            "825",
            "-sha256",
            "-extfile",
            work.join("ext.cnf").to_str().ok_or("non-UTF-8 path")?,
        ],
    )?;

    let chain = std::fs::read(work.join("server.crt"))?;
    let ca = std::fs::read(&ca_cert)?;
    let mut fullchain = chain;
    fullchain.extend_from_slice(&ca);
    atomic_write(&cert, &fullchain, 0o600)?;
    let server_key = std::fs::read(work.join("key.pem"))?;
    atomic_write(&key, &server_key, 0o600)?;
    atomic_write(&sans_file, fingerprint.as_bytes(), 0o600)?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}

fn run_command(program: &str, arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(program)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(format!("{program} exited with {status}").into());
    }
    Ok(())
}

fn reload_nginx(_config: &Config) -> io::Result<()> {
    let status = Command::new("/usr/bin/systemctl")
        .args(["reload", "nginx"])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "nginx reload exited with {status}"
        )))
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|value| {
            value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // Safety: getsockopt writes exactly one ucred into the provided buffer.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut libc::c_void,
            &mut length,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(credentials.uid)
}

fn uid_for_user(name: &str) -> Option<u32> {
    let name = CString::new(name).ok()?;
    // Safety: getpwnam returns a pointer into static storage; the value is
    // copied immediately and no other thread calls it concurrently.
    let entry = unsafe { libc::getpwnam(name.as_ptr()) };
    if entry.is_null() {
        None
    } else {
        Some(unsafe { (*entry).pw_uid })
    }
}

fn gid_for_group(name: &str) -> Option<u32> {
    let name = CString::new(name).ok()?;
    let entry = unsafe { libc::getgrnam(name.as_ptr()) };
    if entry.is_null() {
        None
    } else {
        Some(unsafe { (*entry).gr_gid })
    }
}

fn configure_socket(config: &Config) -> io::Result<()> {
    for directory in [config.socket.parent(), config.ports_file.parent()]
        .into_iter()
        .flatten()
    {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o750))?;
        if let Some(gid) = config.socket_gid {
            chown(directory, 0, gid)?;
        }
    }
    std::fs::set_permissions(&config.socket, std::fs::Permissions::from_mode(0o660))?;
    if let Some(gid) = config.socket_gid {
        chown(&config.socket, 0, gid)?;
    }
    Ok(())
}

fn chown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    if unsafe { libc::chown(path.as_ptr(), uid, gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn parse_args(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Config, Box<dyn std::error::Error>> {
    let mut socket = None;
    let mut state_dir = None;
    let mut app_conf_dir = None;
    let mut ports_file = None;
    let mut tls_dir = None;
    let mut ca_dir = None;
    let mut hosts_file = None;
    let mut ips_file = None;
    let mut platform_user = None;
    let mut socket_group = Some("rumahl-runtime-control".to_owned());
    let mut arguments = arguments.into_iter();

    while let Some(option) = arguments.next() {
        let option = option.to_str().ok_or("non-UTF-8 option")?;
        let value = arguments.next().ok_or("missing option value")?;
        match option {
            "--socket" => socket = Some(PathBuf::from(value)),
            "--state-dir" => state_dir = Some(PathBuf::from(value)),
            "--app-conf-dir" => app_conf_dir = Some(PathBuf::from(value)),
            "--ports-file" => ports_file = Some(PathBuf::from(value)),
            "--tls-dir" => tls_dir = Some(PathBuf::from(value)),
            "--ca-dir" => ca_dir = Some(PathBuf::from(value)),
            "--hosts-file" => hosts_file = Some(PathBuf::from(value)),
            "--ips-file" => ips_file = Some(PathBuf::from(value)),
            "--platform-user" => {
                platform_user = Some(value.into_string().map_err(|_| "non-UTF-8 platform user")?);
            }
            "--socket-group" => {
                socket_group = Some(value.into_string().map_err(|_| "non-UTF-8 socket group")?);
            }
            _ => return Err(format!("unknown option {option}").into()),
        }
    }

    let socket = socket.ok_or("missing --socket")?;
    if !socket.is_absolute() {
        return Err("--socket must be absolute".into());
    }
    let platform_user = platform_user.ok_or("missing --platform-user")?;
    let platform_uid = uid_for_user(&platform_user).ok_or("unknown platform user")?;
    let socket_gid = socket_group.as_deref().and_then(gid_for_group);

    Ok(Config {
        socket,
        state_dir: state_dir.ok_or("missing --state-dir")?,
        app_conf_dir: app_conf_dir.ok_or("missing --app-conf-dir")?,
        ports_file: ports_file.ok_or("missing --ports-file")?,
        tls_dir: tls_dir.ok_or("missing --tls-dir")?,
        ca_dir: ca_dir.ok_or("missing --ca-dir")?,
        hosts_file: hosts_file.ok_or("missing --hosts-file")?,
        ips_file: ips_file.ok_or("missing --ips-file")?,
        platform_uid,
        socket_gid,
    })
}
