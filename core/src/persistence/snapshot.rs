use crate::PlatformState;

use super::InstalledAppSnapshot;

pub const PLATFORM_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct PlatformSnapshot {
    version: u32,
    installed_apps: Vec<InstalledAppSnapshot>,
}

impl PlatformSnapshot {
    pub fn new(version: u32, installed_apps: Vec<InstalledAppSnapshot>) -> Self {
        Self {
            version,
            installed_apps,
        }
    }

    pub fn capture(state: &PlatformState) -> Self {
        let installed_apps = state
            .installed_apps()
            .apps()
            .iter()
            .map(InstalledAppSnapshot::capture)
            .collect();

        Self::new(PLATFORM_SNAPSHOT_VERSION, installed_apps)
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn installed_apps(&self) -> &[InstalledAppSnapshot] {
        &self.installed_apps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_empty_platform_state() {
        let snapshot = PlatformSnapshot::capture(&PlatformState::new());

        assert_eq!(snapshot.version(), PLATFORM_SNAPSHOT_VERSION);
        assert!(snapshot.installed_apps().is_empty());
    }
}
