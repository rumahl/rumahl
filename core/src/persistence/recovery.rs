use std::error::Error;
use std::fmt;

use crate::{AppLifecycle, AppLifecycleError, PlatformState};

use super::{PLATFORM_SNAPSHOT_VERSION, PlatformSnapshot};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformRecovery {
    lifecycle: AppLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformRecoveryReport {
    installed_apps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformRecoveryError {
    UnsupportedVersion { found: u32, supported: u32 },
    AppRestoreFailed(AppLifecycleError),
}

impl PlatformRecovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recover(
        &self,
        snapshot: &PlatformSnapshot,
        state: &mut PlatformState,
    ) -> Result<PlatformRecoveryReport, PlatformRecoveryError> {
        if snapshot.version() != PLATFORM_SNAPSHOT_VERSION {
            return Err(PlatformRecoveryError::UnsupportedVersion {
                found: snapshot.version(),
                supported: PLATFORM_SNAPSHOT_VERSION,
            });
        }

        let mut recovered = PlatformState::new();

        for snapshot in snapshot.installed_apps() {
            self.lifecycle
                .restore(snapshot.installed_app().clone(), &mut recovered)
                .map_err(PlatformRecoveryError::AppRestoreFailed)?;
        }

        let report = PlatformRecoveryReport {
            installed_apps: recovered.installed_apps().len(),
        };

        *state = recovered;

        Ok(report)
    }
}

impl PlatformRecoveryReport {
    pub fn installed_apps(&self) -> usize {
        self.installed_apps
    }
}

impl fmt::Display for PlatformRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "platform snapshot version {found} is unsupported; expected {supported}"
                )
            }

            Self::AppRestoreFailed(error) => {
                write!(f, "cannot restore installed app: {error}")
            }
        }
    }
}

impl Error for PlatformRecoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AppRestoreFailed(error) => Some(error),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppManifest, AppVersion, CapabilityId, InstalledAppSnapshot, PackagePath,
        PublisherId, RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    fn manifest(app_id: &str, capability_id: &str) -> AppManifest {
        let mut runtime = RuntimeDescriptor::web();

        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        let mut manifest = AppManifest::new(
            AppId::parse(app_id).unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Recovered app",
            runtime,
        )
        .unwrap();

        manifest
            .add_provided_capability(CapabilityId::parse(capability_id).unwrap())
            .unwrap();

        manifest
    }

    fn installed_snapshot(app_id: &str, capability_id: &str) -> InstalledAppSnapshot {
        let mut state = PlatformState::new();
        let app = AppLifecycle::new()
            .install(manifest(app_id, capability_id), &mut state)
            .unwrap();

        InstalledAppSnapshot::capture(&app)
    }

    #[test]
    fn rebuilds_installed_apps_and_derived_registries() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let installation_id = *app.installation_id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, vec![app]);
        let mut state = PlatformState::new();

        let report = PlatformRecovery::new()
            .recover(&snapshot, &mut state)
            .unwrap();

        assert_eq!(report.installed_apps(), 1);
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&installation_id)
                .is_some()
        );
        assert_eq!(state.capability_registry().len(), 1);
    }

    #[test]
    fn rejects_unsupported_version_without_mutating_live_state() {
        let mut state = PlatformState::new();
        let existing = AppLifecycle::new()
            .install(
                manifest("com.rumahl.existing", "rumahl.existing.open"),
                &mut state,
            )
            .unwrap();
        let existing_id = *existing.installation_id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION + 1, Vec::new());

        let error = PlatformRecovery::new()
            .recover(&snapshot, &mut state)
            .unwrap_err();

        assert_eq!(
            error,
            PlatformRecoveryError::UnsupportedVersion {
                found: PLATFORM_SNAPSHOT_VERSION + 1,
                supported: PLATFORM_SNAPSHOT_VERSION,
            }
        );
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&existing_id)
                .is_some()
        );
        assert_eq!(state.capability_registry().len(), 1);
    }

    #[test]
    fn rejects_conflicting_snapshot_without_mutating_live_state() {
        let duplicate = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let snapshot = PlatformSnapshot::new(
            PLATFORM_SNAPSHOT_VERSION,
            vec![duplicate.clone(), duplicate],
        );
        let mut state = PlatformState::new();
        let existing = AppLifecycle::new()
            .install(
                manifest("com.rumahl.existing", "rumahl.existing.open"),
                &mut state,
            )
            .unwrap();
        let existing_id = *existing.installation_id();

        assert!(matches!(
            PlatformRecovery::new().recover(&snapshot, &mut state),
            Err(PlatformRecoveryError::AppRestoreFailed(
                AppLifecycleError::InstalledAppConflict(_)
            ))
        ));
        assert_eq!(state.installed_apps().len(), 1);
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&existing_id)
                .is_some()
        );
        assert_eq!(state.capability_registry().len(), 1);
    }

    #[test]
    fn empty_snapshot_replaces_existing_state() {
        let mut state = PlatformState::new();
        AppLifecycle::new()
            .install(
                manifest("com.rumahl.existing", "rumahl.existing.open"),
                &mut state,
            )
            .unwrap();

        let report = PlatformRecovery::new()
            .recover(
                &PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, Vec::new()),
                &mut state,
            )
            .unwrap();

        assert_eq!(report.installed_apps(), 0);
        assert!(state.installed_apps().is_empty());
        assert!(state.capability_registry().is_empty());
    }
}
