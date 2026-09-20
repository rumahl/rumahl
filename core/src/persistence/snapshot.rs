use crate::{InMemoryGrantStore, PlatformState};

use super::{InstalledAppSnapshot, PermissionGrantSnapshot};

pub const PLATFORM_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct PlatformSnapshot {
    version: u32,
    installed_apps: Vec<InstalledAppSnapshot>,
    grants: Vec<PermissionGrantSnapshot>,
}

impl PlatformSnapshot {
    pub fn new(
        version: u32,
        installed_apps: Vec<InstalledAppSnapshot>,
        grants: Vec<PermissionGrantSnapshot>,
    ) -> Self {
        Self {
            version,
            installed_apps,
            grants,
        }
    }

    pub fn capture(state: &PlatformState, grant_store: &InMemoryGrantStore) -> Self {
        let installed_apps = state
            .installed_apps()
            .apps()
            .iter()
            .map(InstalledAppSnapshot::capture)
            .collect();
        let grants = grant_store
            .grants()
            .iter()
            .map(PermissionGrantSnapshot::capture)
            .collect();

        Self::new(PLATFORM_SNAPSHOT_VERSION, installed_apps, grants)
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn installed_apps(&self) -> &[InstalledAppSnapshot] {
        &self.installed_apps
    }

    pub fn grants(&self) -> &[PermissionGrantSnapshot] {
        &self.grants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_empty_platform_state() {
        let snapshot = PlatformSnapshot::capture(&PlatformState::new(), &InMemoryGrantStore::new());

        assert_eq!(snapshot.version(), PLATFORM_SNAPSHOT_VERSION);
        assert!(snapshot.installed_apps().is_empty());
        assert!(snapshot.grants().is_empty());
    }
}
