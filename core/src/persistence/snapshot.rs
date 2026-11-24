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
        // Session grants are deliberately absent: the snapshot is the restart
        // boundary and a consented-this-session grant must never survive it.
        let grants = grant_store
            .grants()
            .iter()
            .filter(|grant| grant.validity().is_durable())
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

    use crate::{
        AppId, AppIdentity, GrantAuthority, GrantIssuerPolicy, GrantValidity, Identity,
        InstallationId, PermissionId, PermissionScope, PublisherId, SessionId, UserIdentity,
        UserRole,
    };

    fn app_identity() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn captures_empty_platform_state() {
        let snapshot = PlatformSnapshot::capture(&PlatformState::new(), &InMemoryGrantStore::new());

        assert_eq!(snapshot.version(), PLATFORM_SNAPSHOT_VERSION);
        assert!(snapshot.installed_apps().is_empty());
        assert!(snapshot.grants().is_empty());
    }

    #[test]
    fn session_grants_never_reach_the_snapshot() {
        let app = app_identity();
        let user = UserIdentity::new(crate::UserId::new());
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(*user.id(), UserRole::User);
        let permission = PermissionId::parse("rumahl.files.read").unwrap();

        let durable = GrantAuthority::new()
            .issue(
                &policy,
                Identity::User(user),
                Identity::App(app.clone()),
                permission.clone(),
                PermissionScope::Explicit,
                vec![crate::ResourceRef::new(
                    crate::ResourceNamespace::parse("rumahl.files").unwrap(),
                    crate::ResourceKind::parse("file").unwrap(),
                    crate::ResourceKey::parse("document-1").unwrap(),
                )],
            )
            .unwrap();
        let session = GrantAuthority::new()
            .issue_with_validity(
                &policy,
                Identity::User(user),
                Identity::App(app),
                permission,
                PermissionScope::Explicit,
                vec![crate::ResourceRef::new(
                    crate::ResourceNamespace::parse("rumahl.files").unwrap(),
                    crate::ResourceKind::parse("file").unwrap(),
                    crate::ResourceKey::parse("document-2").unwrap(),
                )],
                GrantValidity::session(SessionId::new()),
            )
            .unwrap();

        let mut grants = InMemoryGrantStore::new();
        grants.insert(durable);
        grants.insert(session);

        let snapshot = PlatformSnapshot::capture(&PlatformState::new(), &grants);

        assert_eq!(snapshot.grants().len(), 1);
        // The persisted grant keeps its own validity, so an expiry would
        // survive the restart and still be enforced after it.
        assert_eq!(snapshot.grants()[0].validity(), &GrantValidity::durable());
    }
}
