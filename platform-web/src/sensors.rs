//! Host sensors: load, memory, temperature and uptime, with severity levels.
//!
//! Read on Linux from `/proc` and `/sys`; other platforms report `unknown` so
//! the recovery hub still renders. Values are best-effort and never fail.

use serde::Serialize;

#[derive(Serialize)]
pub struct Metric {
    pub id: &'static str,
    pub label: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub level: &'static str,
}

#[derive(Serialize)]
pub struct Sensors {
    #[serde(rename = "sensorsVersion")]
    pub version: u8,
    pub host: Option<String>,
    pub uptime: Option<f64>,
    pub cores: usize,
    pub level: &'static str,
    pub at: u64,
    pub metrics: Vec<Metric>,
}

fn level_for(value: f64, warn: f64, error: f64) -> &'static str {
    if value >= error {
        "error"
    } else if value >= warn {
        "warning"
    } else {
        "information"
    }
}

fn worst<'a>(levels: impl IntoIterator<Item = &'a str>) -> &'static str {
    let mut result = "information";
    for level in levels {
        result = match (result, level) {
            (_, "error") => "error",
            ("error", _) => "error",
            (_, "warning") => "warning",
            ("warning", _) => "warning",
            _ => result,
        };
    }
    result
}

fn read_loadavg() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    text.split_whitespace().next()?.parse().ok()
}

fn read_meminfo() -> Option<(f64, f64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total = parts.next().and_then(|v| v.parse::<f64>().ok()),
            Some("MemAvailable:") => available = parts.next().and_then(|v| v.parse::<f64>().ok()),
            _ => {}
        }
    }
    Some((total?, available.unwrap_or(0.0)))
}

fn read_uptime() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    text.split_whitespace().next()?.parse().ok()
}

fn read_temperature() -> Option<f64> {
    for zone in 0..8 {
        let path = format!("/sys/class/thermal/thermal_zone{zone}/temp");
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(milli) = text.trim().parse::<f64>()
        {
            return Some(milli / 1000.0);
        }
    }
    None
}

fn read_hostname() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Serialize)]
pub struct Storage {
    #[serde(rename = "recoveryVersion")]
    pub version: u8,
    pub mount: Option<String>,
    pub total: Option<u64>,
    pub used: Option<u64>,
    pub available: Option<u64>,
    pub percent: Option<f64>,
    pub level: &'static str,
}

/// Disk usage of the root filesystem (best-effort, via `df` on Linux).
pub fn read_storage() -> Storage {
    let mut storage = Storage {
        version: 1,
        mount: None,
        total: None,
        used: None,
        available: None,
        percent: None,
        level: "information",
    };
    if let Ok(output) = std::process::Command::new("df")
        .arg("-Pk")
        .arg("/")
        .output()
        && let Ok(text) = String::from_utf8(output.stdout)
        && let Some(line) = text.lines().nth(1)
    {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 6 {
            let total_kb: u64 = fields[1].parse().unwrap_or(0);
            let used_kb: u64 = fields[2].parse().unwrap_or(0);
            let available_kb: u64 = fields[3].parse().unwrap_or(0);
            let percent = if total_kb > 0 {
                used_kb as f64 / total_kb as f64 * 100.0
            } else {
                0.0
            };
            storage.mount = Some(fields[5].to_owned());
            storage.total = Some(total_kb * 1024);
            storage.used = Some(used_kb * 1024);
            storage.available = Some(available_kb * 1024);
            storage.percent = Some((percent * 10.0).round() / 10.0);
            storage.level = level_for(percent, 75.0, 90.0);
        }
    }
    storage
}

pub fn read_sensors() -> Sensors {
    let cores = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let mut metrics = Vec::new();
    if let Some((total, available)) = read_meminfo()
        && total > 0.0
    {
        let used = (total - available) / total * 100.0;
        metrics.push(Metric {
            id: "memory",
            label: "Memory",
            value: (used * 10.0).round() / 10.0,
            unit: "%",
            level: level_for(used, 75.0, 90.0),
        });
    }
    if let Some(load) = read_loadavg() {
        // Load relative to the core count, as a percentage.
        let percent = load / cores as f64 * 100.0;
        metrics.push(Metric {
            id: "cpu",
            label: "CPU load",
            value: (percent * 10.0).round() / 10.0,
            unit: "%",
            level: level_for(percent, 100.0, 150.0),
        });
    }
    if let Some(temperature) = read_temperature() {
        metrics.push(Metric {
            id: "temperature",
            label: "Temperature",
            value: (temperature * 10.0).round() / 10.0,
            unit: "°C",
            level: level_for(temperature, 70.0, 85.0),
        });
    }
    Sensors {
        version: 1,
        host: read_hostname(),
        uptime: read_uptime(),
        cores,
        level: worst(metrics.iter().map(|metric| metric.level)),
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or(0),
        metrics,
    }
}

#[derive(Serialize)]
pub struct DiskDevice {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub mount: Option<String>,
    pub fstype: Option<String>,
    pub model: Option<String>,
    pub percent: Option<f64>,
    pub level: &'static str,
}

fn flatten_blockdevices(node: &serde_json::Value, out: &mut Vec<DiskDevice>) {
    let string = |key: &str| {
        node.get(key)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    };
    let percent =
        string("fsuse%").and_then(|value| value.trim_end_matches('%').trim().parse::<f64>().ok());
    out.push(DiskDevice {
        name: string("name").unwrap_or_default(),
        kind: string("type").unwrap_or_default(),
        size: node
            .get("size")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        mount: string("mountpoint").filter(|value| !value.is_empty()),
        fstype: string("fstype").filter(|value| !value.is_empty()),
        model: string("model")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        percent,
        level: level_for(percent.unwrap_or(0.0), 75.0, 90.0),
    });
    if let Some(children) = node.get("children").and_then(|value| value.as_array()) {
        for child in children {
            flatten_blockdevices(child, out);
        }
    }
}

/// Block devices and partitions (best-effort, via `lsblk -J` on Linux).
pub fn read_disks() -> Vec<DiskDevice> {
    let mut devices = Vec::new();
    if let Ok(output) = std::process::Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,TYPE,SIZE,MOUNTPOINT,FSTYPE,MODEL,FSUSE%",
        ])
        .output()
        && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        && let Some(list) = json.get("blockdevices").and_then(|value| value.as_array())
    {
        for device in list {
            flatten_blockdevices(device, &mut devices);
        }
    }
    devices
}

/// Runs a SMART self-test on a device (best-effort; needs `smartctl` + access).
pub fn run_disk_test(name: &str, test: &str) -> bool {
    if name.is_empty()
        || name.len() > 128
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
    {
        return false;
    }
    let test = if test == "long" { "long" } else { "short" };
    std::process::Command::new("smartctl")
        .args(["-t", test, &format!("/dev/{name}")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
