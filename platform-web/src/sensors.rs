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
