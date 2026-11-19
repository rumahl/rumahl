//! Read-only recovery diagnostics: network, logs, boot and build information.
//!
//! Everything here is best-effort on Linux and never fails: a missing tool or a
//! denied file yields an empty/`available: false` result so the recovery
//! environment still renders and explains what it could not read.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

/// Runs a short-lived command and returns its stdout on success.
fn run(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(args[0])
        .args(&args[1..])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Resolves a host with a hard wall-clock bound so a dead resolver cannot hang
/// the recovery surface.
fn resolve(host: &str, port: u16, timeout: Duration) -> Vec<SocketAddr> {
    let (sender, receiver) = mpsc::channel();
    let host = host.to_owned();
    std::thread::spawn(move || {
        let result = (host.as_str(), port)
            .to_socket_addrs()
            .map(|addrs| addrs.collect())
            .unwrap_or_default();
        let _ = sender.send(result);
    });
    receiver.recv_timeout(timeout).unwrap_or_default()
}

/// A TCP handshake to `host:port`. Reachability is proven without ICMP/raw
/// sockets so it works for an unprivileged platform service.
fn reachable(host: &str, port: u16) -> bool {
    resolve(host, port, Duration::from_secs(2))
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok())
}

fn text(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

struct Interface {
    name: String,
    state: String,
    mac: Option<String>,
    addresses: Vec<String>,
}

fn read_interfaces() -> Vec<Interface> {
    // Prefer `ip -j` for a complete picture, fall back to the kernel sysfs view.
    if let Some(raw) = run(&["ip", "-j", "addr", "show"])
        && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&raw)
    {
        let mut interfaces = Vec::new();
        for item in items {
            let name = item
                .get("ifname")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if name.is_empty() || name == "lo" {
                continue;
            }
            let state = item
                .get("operstate")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            let mac = item
                .get("address")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && *value != "00:00:00:00:00:00")
                .map(str::to_owned);
            let addresses = item
                .get("addr_info")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            let local = entry.get("local").and_then(Value::as_str)?;
                            let prefix = entry.get("prefixlen").and_then(Value::as_u64);
                            let family = entry
                                .get("family")
                                .and_then(Value::as_str)
                                .unwrap_or("inet");
                            Some(match prefix {
                                Some(prefix) => format!("{local}/{prefix}"),
                                None => format!("{local} ({family})"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            interfaces.push(Interface {
                name,
                state,
                mac,
                addresses,
            });
        }
        return interfaces;
    }
    let mut interfaces = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "lo" {
                continue;
            }
            let state = text(&format!("/sys/class/net/{name}/operstate"))
                .map(|value| value.trim().to_owned())
                .unwrap_or_else(|| "unknown".to_owned());
            let mac = text(&format!("/sys/class/net/{name}/address"))
                .map(|value| value.trim().to_owned())
                .filter(|value| value != "00:00:00:00:00:00");
            interfaces.push(Interface {
                name,
                state,
                mac,
                addresses: Vec::new(),
            });
        }
    }
    interfaces
}

fn read_default_gateway() -> Option<String> {
    if let Some(raw) = run(&["ip", "-j", "route", "show", "default"])
        && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&raw)
        && let Some(gateway) = items
            .first()
            .and_then(|item| item.get("gateway"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
    {
        return Some(gateway.to_owned());
    }
    let raw = text("/proc/net/route")?;
    for line in raw.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[1] == "00000000" && fields[2] != "00000000" {
            // Destination 0.0.0.0 is little-endian hex, so reverse the bytes.
            let hex = fields[2];
            if hex.len() == 8 {
                let bytes: Vec<u8> = (0..4)
                    .rev()
                    .filter_map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok())
                    .collect();
                if bytes.len() == 4 {
                    return Some(format!(
                        "{}.{}.{}.{}",
                        bytes[0], bytes[1], bytes[2], bytes[3]
                    ));
                }
            }
        }
    }
    None
}

fn read_dns() -> Vec<String> {
    let mut servers = Vec::new();
    if let Some(raw) = text("/etc/resolv.conf") {
        for line in raw.lines() {
            let line = line.trim();
            if let Some(address) = line.strip_prefix("nameserver") {
                let address = address.trim().split(['#', ' ']).next().unwrap_or("").trim();
                if !address.is_empty() && !servers.iter().any(|entry| entry == address) {
                    servers.push(address.to_owned());
                }
            }
        }
    }
    servers
}

/// Interfaces, addresses, default gateway and resolvers. Read-only.
pub fn read_network() -> Value {
    let interfaces: Vec<Value> = read_interfaces()
        .into_iter()
        .map(|interface| {
            json!({
                "name": interface.name,
                "state": interface.state,
                "mac": interface.mac,
                "addresses": interface.addresses,
            })
        })
        .collect();
    let gateway = read_default_gateway();
    let dns = read_dns();
    let level = if interfaces
        .iter()
        .any(|interface| interface["state"] == "up")
    {
        "information"
    } else if interfaces.is_empty() {
        "warning"
    } else {
        "error"
    };
    json!({
        "recoveryVersion": 1,
        "hostname": text("/proc/sys/kernel/hostname").map(|value| value.trim().to_owned()),
        "interfaces": interfaces,
        "gateway": gateway,
        "dns": dns,
        "level": level,
    })
}

/// Active connectivity probes against the gateway, the resolver and the public
/// internet, plus a real DNS lookup. Read-only.
pub fn run_network_test() -> Value {
    let gateway = read_default_gateway();
    let dns = read_dns();
    let mut checks = Vec::new();
    if let Some(gateway) = gateway.clone() {
        checks.push(json!({"id": "gateway", "target": gateway, "ok": reachable(&gateway, 443) || reachable(&gateway, 80) || reachable(&gateway, 53)}));
    } else {
        checks.push(json!({"id": "gateway", "target": null, "ok": false}));
    }
    match dns.first() {
        Some(server) => {
            checks.push(json!({"id": "dns-server", "target": server, "ok": reachable(server, 53)}))
        }
        None => checks.push(json!({"id": "dns-server", "target": null, "ok": false})),
    }
    let lookup_ok = !resolve("one.one.one.one", 443, Duration::from_secs(3)).is_empty();
    checks.push(json!({"id": "resolver", "target": "one.one.one.one", "ok": lookup_ok}));
    checks.push(json!({"id": "internet", "target": "1.1.1.1:53", "ok": reachable("1.1.1.1", 53) || reachable("8.8.8.8", 53)}));
    let ok = checks.iter().all(|check| check["ok"] == true);
    json!({ "recoveryVersion": 1, "ok": ok, "at": now_seconds(), "checks": checks })
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Recent platform/system log lines. `level` maps to a syslog priority floor.
pub fn read_logs(source: &str, level: Option<&str>, limit: usize, offset: usize) -> Value {
    let unit = match source {
        "system" => None,
        _ => Some("rumahl-platform.service"),
    };
    let priority = match level {
        Some("error") => Some("err"),
        Some("warning") => Some("warning"),
        Some("critical") => Some("crit"),
        _ => None,
    };
    let want = (limit + offset).clamp(1, 1000).to_string();
    let mut args: Vec<&str> = vec![
        "journalctl",
        "--no-pager",
        "--no-hostname",
        "-o",
        "short-iso",
    ];
    if let Some(unit) = unit {
        args.push("-u");
        args.push(unit);
    }
    if let Some(priority) = priority {
        args.push("-p");
        args.push(priority);
    }
    args.push("-n");
    args.push(&want);
    let Some(raw) = run(&args) else {
        return json!({ "recoveryVersion": 1, "available": false, "source": source, "entries": [] });
    };
    let mut lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    lines.reverse();
    let entries: Vec<Value> = lines
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|line| {
            let (at, message) = match line.find(' ') {
                Some(index) => (
                    line[..index].to_owned(),
                    line[index + 1..].trim().to_owned(),
                ),
                None => (String::new(), line.trim().to_owned()),
            };
            json!({ "at": at, "message": message })
        })
        .collect();
    json!({ "recoveryVersion": 1, "available": true, "source": source, "entries": entries })
}

/// Why the device may have needed recovery: failed units and boot errors.
pub fn read_boot() -> Value {
    let failed: Vec<Value> = run(&[
        "systemctl",
        "--failed",
        "--no-legend",
        "--plain",
        "--no-pager",
    ])
    .map(|raw| {
        raw.lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let unit = fields.next()?.to_owned();
                let load = fields.next().unwrap_or_default().to_owned();
                let active = fields.next().unwrap_or_default().to_owned();
                Some(json!({ "unit": unit, "load": load, "active": active }))
            })
            .collect()
    })
    .unwrap_or_default();
    let errors: Vec<String> = run(&[
        "journalctl",
        "--no-pager",
        "-b",
        "-p",
        "err",
        "-n",
        "100",
        "-o",
        "cat",
    ])
    .map(|raw| {
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default();
    let uptime =
        text("/proc/uptime").and_then(|value| value.split_whitespace().next()?.parse::<f64>().ok());
    json!({
        "recoveryVersion": 1,
        "at": now_seconds(),
        "uptime": uptime,
        "kernel": text("/proc/sys/kernel/osrelease").map(|value| value.trim().to_owned()),
        "failedUnits": failed,
        "errors": errors,
    })
}

/// Build and system identification. Read-only; the rollback lane is served by
/// the host broker, not here.
pub fn read_system() -> Value {
    let mut release = serde_json::Map::new();
    if let Some(raw) = text("/etc/os-release") {
        for line in raw.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                if matches!(
                    key,
                    "NAME" | "VERSION" | "VERSION_ID" | "PRETTY_NAME" | "BUILD_ID"
                ) {
                    release.insert(key.to_owned(), Value::String(value.to_owned()));
                }
            }
        }
    }
    let build = text("/etc/rumahl/build")
        .or_else(|| text("/usr/lib/rumahl/build"))
        .map(|value| value.trim().to_owned());
    json!({
        "recoveryVersion": 1,
        "at": now_seconds(),
        "osRelease": Value::Object(release),
        "build": build,
        "kernel": text("/proc/sys/kernel/osrelease").map(|value| value.trim().to_owned()),
        "machineId": text("/etc/machine-id").map(|value| value.trim().to_owned()),
        "hostname": text("/proc/sys/kernel/hostname").map(|value| value.trim().to_owned()),
    })
}
