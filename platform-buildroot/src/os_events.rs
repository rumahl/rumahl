//! Reports OS-layer (Buildroot) events into the central audit spool.
//!
//! The platform ingests `os-events.jsonl` and attributes every line to
//! "rumahl OS", so users never see an internal layer name.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::os_layout::STATE_ROOT;

/// Path of the OS event spool for a state directory.
pub fn os_event_spool_path(state_dir: &Path) -> PathBuf {
    state_dir.join("os-events.jsonl")
}

/// Appends one OS event to a specific spool. Best-effort: never panics.
pub fn append_os_event(path: &Path, action: &str, outcome: &str, target: Option<&str>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let escape = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
    let line = match target {
        Some(target) => format!(
            "{{\"action\":\"{}\",\"outcome\":\"{}\",\"target\":\"{}\"}}",
            escape(action),
            escape(outcome),
            escape(target)
        ),
        None => format!(
            "{{\"action\":\"{}\",\"outcome\":\"{}\"}}",
            escape(action),
            escape(outcome)
        ),
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// Reports an OS event using the state directory from `RUMAHL_STATE_DIR`.
pub fn report_os_event(action: &str, outcome: &str, target: Option<&str>) {
    let state_dir = std::env::var_os("RUMAHL_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(STATE_ROOT));
    append_os_event(&os_event_spool_path(&state_dir), action, outcome, target);
}
