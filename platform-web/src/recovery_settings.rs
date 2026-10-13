//! Recovery environment settings (boot/repair behaviour), stored per device.
//!
//! Kept in a small JSON file in the state directory so the recovery surface can
//! read and change how rumahl OS reacts to errors.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverySettings {
    /// Automatically restart a crashed/unresponsive app runtime.
    pub auto_repair: bool,
    /// Boot into a safe mode that keeps third-party apps stopped.
    pub safe_mode: bool,
    /// Apps quarantined after repeated crashes. Never auto-restarted.
    pub disabled_apps: Vec<String>,
}

impl Default for RecoverySettings {
    fn default() -> Self {
        Self {
            auto_repair: true,
            safe_mode: false,
            disabled_apps: Vec::new(),
        }
    }
}

/// Whether an app is quarantined and must not be auto-restarted.
pub fn is_disabled(settings: &RecoverySettings, app_id: &str) -> bool {
    settings.disabled_apps.iter().any(|entry| entry == app_id)
}

fn path(state_dir: &Path) -> PathBuf {
    state_dir.join("recovery-settings.json")
}

pub fn load(state_dir: Option<&Path>) -> RecoverySettings {
    let Some(dir) = state_dir else {
        return RecoverySettings::default();
    };
    std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(state_dir: Option<&Path>, settings: &RecoverySettings) -> bool {
    let Some(dir) = state_dir else {
        return false;
    };
    let _ = std::fs::create_dir_all(dir);
    std::fs::write(
        path(dir),
        serde_json::to_string_pretty(settings).unwrap_or_default(),
    )
    .is_ok()
}
