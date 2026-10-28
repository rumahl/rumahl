use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::{
    AppLifecycle, AppLifecycleError, GrantId, Identity, InMemoryGrantStore, PermissionGrant,
    PlatformState,
};

use super::{PLATFORM_SNAPSHOT_VERSION, PlatformSnapshot};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformRecovery {
    lifecycle: AppLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformRecoveryReport {
    installed_apps: usize,
    grants: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformRecoveryError {
    UnsupportedVersion { found: u32, supported: u32 },
    AppRestoreFailed(AppLifecycleError),
    DuplicateGrantId(GrantId),
    GrantSubjectNotInstalled(GrantId),
    AppCannotIssueGrant(GrantId),
}

impl PlatformRecovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recover(
        &self,
        snapshot: &PlatformSnapshot,
        state: &mut PlatformState,
        grant_store: &mut InMemoryGrantStore,
    ) -> Result<PlatformRecoveryReport, PlatformRecoveryError> {
        if snapshot.version() != PLATFORM_SNAPSHOT_VERSION {
            return Err(PlatformRecoveryError::UnsupportedVersion {
                found: snapshot.version(),
                supported: PLATFORM_SNAPSHOT_VERSION,
            });
        }

        let mut recovered = PlatformState::new();
        let mut recovered_grants = InMemoryGrantStore::new();
        let mut grant_ids = HashSet::new();

        for snapshot in snapshot.installed_apps() {
            self.lifecycle
                .restore(snapshot.installed_app().clone(), &mut recovered)
                .map_err(PlatformRecoveryError::AppRestoreFailed)?;
        }

        for snapshot in snapshot.grants() {
            let grant = snapshot.grant();

            if !grant_ids.insert(*grant.id()) {
                return Err(PlatformRecoveryError::DuplicateGrantId(*grant.id()));
            }

            Self::validate_grant(grant, &recovered)?;
            recovered_grants.insert(grant.clone());
        }

        let report = PlatformRecoveryReport {
            installed_apps: recovered.installed_apps().len(),
            grants: recovered_grants.len(),
        };

        *state = recovered;
        *grant_store = recovered_grants;

        Ok(report)
    }

    fn validate_grant(
        grant: &PermissionGrant,
        state: &PlatformState,
    ) -> Result<(), PlatformRecoveryError> {
        if let Identity::App(subject) = grant.subject() {
            let installed = state
                .installed_apps()
                .get_by_installation_id(subject.installation_id())
                .filter(|installed| installed.identity() == subject);

            if installed.is_none() {
                return Err(PlatformRecoveryError::GrantSubjectNotInstalled(*grant.id()));
            }
        }

        if grant.granted_by().is_app() {
            return Err(PlatformRecoveryError::AppCannotIssueGrant(*grant.id()));
        }

        Ok(())
    }
}

impl PlatformRecoveryReport {
    pub fn installed_apps(&self) -> usize {
        self.installed_apps
    }

    pub fn grants(&self) -> usize {
        self.grants
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

            Self::DuplicateGrantId(id) => {
                write!(f, "platform snapshot contains duplicate grant ID {id}")
            }

            Self::GrantSubjectNotInstalled(id) => {
                write!(
                    f,
                    "grant {id} references an app installation that is not installed"
                )
            }

            Self::AppCannotIssueGrant(id) => {
                write!(f, "grant {id} was issued by an app identity")
            }
        }
    }
}

impl Error for PlatformRecoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AppRestoreFailed(error) => Some(error),
            Self::UnsupportedVersion { .. }
            | Self::DuplicateGrantId(_)
            | Self::GrantSubjectNotInstalled(_)
            | Self::AppCannotIssueGrant(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, AppManifest, AppVersion, CapabilityId, GrantId, Identity,
        InstalledAppSnapshot, PackagePath, PermissionGrantSnapshot, PermissionId, PermissionScope,
        PublisherId, ResourceKey, ResourceKind, ResourceNamespace, ResourceRef, RuntimeDescriptor,
        RuntimeEntrypoint, RuntimeEntrypointId, UserId, UserIdentity,
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

    fn grant_snapshot(subject: Identity, granted_by: Identity) -> PermissionGrantSnapshot {
        PermissionGrantSnapshot::new(
            GrantId::new(),
            subject,
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![ResourceRef::new(
                ResourceNamespace::parse("rumahl.files").unwrap(),
                ResourceKind::parse("file").unwrap(),
                ResourceKey::parse("document-1").unwrap(),
            )],
            granted_by,
        )
        .unwrap()
    }

    #[test]
    fn rebuilds_installed_apps_and_derived_registries() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let installation_id = *app.installation_id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, vec![app], Vec::new());
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        let report = PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .unwrap();

        assert_eq!(report.installed_apps(), 1);
        assert_eq!(report.grants(), 0);
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
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION + 1, Vec::new(), Vec::new());
        let mut grants = InMemoryGrantStore::new();

        let error = PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
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
            Vec::new(),
        );
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();
        let existing = AppLifecycle::new()
            .install(
                manifest("com.rumahl.existing", "rumahl.existing.open"),
                &mut state,
            )
            .unwrap();
        let existing_id = *existing.installation_id();

        assert!(matches!(
            PlatformRecovery::new().recover(&snapshot, &mut state, &mut grants),
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
                &PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, Vec::new(), Vec::new()),
                &mut state,
                &mut InMemoryGrantStore::new(),
            )
            .unwrap();

        assert_eq!(report.installed_apps(), 0);
        assert!(state.installed_apps().is_empty());
        assert!(state.capability_registry().is_empty());
    }

    #[test]
    fn restores_grant_for_exact_installed_app_identity() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let issuer = UserIdentity::new(UserId::new());
        let grant = grant_snapshot(app.identity().clone().into(), issuer.into());
        let grant_id = *grant.id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, vec![app], vec![grant]);
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        let report = PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .unwrap();

        assert_eq!(report.grants(), 1);
        assert!(grants.get(&grant_id).is_some());
    }

    #[test]
    fn rejects_grant_for_stale_app_without_mutating_live_state_or_grants() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let stale_subject = AppIdentity::new(
            app.identity().app_id().clone(),
            crate::InstallationId::new(),
            app.identity().publisher_id().clone(),
        );
        let grant = grant_snapshot(
            stale_subject.into(),
            UserIdentity::new(UserId::new()).into(),
        );
        let rejected_grant_id = *grant.id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, vec![app], vec![grant]);
        let mut state = PlatformState::new();
        let existing = AppLifecycle::new()
            .install(
                manifest("com.rumahl.existing", "rumahl.existing.open"),
                &mut state,
            )
            .unwrap();
        let existing_id = *existing.installation_id();
        let mut grants = InMemoryGrantStore::new();
        let existing_grant = grant_snapshot(
            UserIdentity::new(UserId::new()).into(),
            UserIdentity::new(UserId::new()).into(),
        );
        let existing_grant_id = *existing_grant.id();
        grants.insert(existing_grant.grant().clone());

        assert_eq!(
            PlatformRecovery::new().recover(&snapshot, &mut state, &mut grants),
            Err(PlatformRecoveryError::GrantSubjectNotInstalled(
                rejected_grant_id
            ))
        );
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&existing_id)
                .is_some()
        );
        assert_eq!(grants.len(), 1);
        assert!(grants.get(&existing_grant_id).is_some());
    }

    #[test]
    fn rejects_duplicate_grant_ids() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let grant = grant_snapshot(
            app.identity().clone().into(),
            UserIdentity::new(UserId::new()).into(),
        );
        let grant_id = *grant.id();
        let snapshot = PlatformSnapshot::new(
            PLATFORM_SNAPSHOT_VERSION,
            vec![app],
            vec![grant.clone(), grant],
        );

        assert_eq!(
            PlatformRecovery::new().recover(
                &snapshot,
                &mut PlatformState::new(),
                &mut InMemoryGrantStore::new(),
            ),
            Err(PlatformRecoveryError::DuplicateGrantId(grant_id))
        );
    }

    #[test]
    fn rejects_grant_issued_by_app_identity() {
        let app = installed_snapshot("com.rumahl.notes", "rumahl.notes.open");
        let app_identity: Identity = app.identity().clone().into();
        let grant = grant_snapshot(app_identity.clone(), app_identity);
        let grant_id = *grant.id();
        let snapshot = PlatformSnapshot::new(PLATFORM_SNAPSHOT_VERSION, vec![app], vec![grant]);

        assert_eq!(
            PlatformRecovery::new().recover(
                &snapshot,
                &mut PlatformState::new(),
                &mut InMemoryGrantStore::new(),
            ),
            Err(PlatformRecoveryError::AppCannotIssueGrant(grant_id))
        );
    }
}
