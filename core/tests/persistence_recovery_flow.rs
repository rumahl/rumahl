use std::cell::RefCell;
use std::convert::Infallible;

use rumahl_core::{
    AppId, AppLifecycle, AppManifest, AppVersion, GrantAuthority, GrantIssuerPolicy,
    InMemoryGrantStore, PackagePath, PermissionId, PermissionScope, PlatformPersistence,
    PlatformSnapshot, PlatformSnapshotRepository, PlatformState, PublisherId, ResourceKey,
    ResourceKind, ResourceNamespace, ResourceRef, RuntimeDescriptor, RuntimeEntrypoint,
    RuntimeEntrypointId, UserId, UserIdentity, UserRole,
};

#[derive(Default)]
struct MemorySnapshotRepository {
    snapshot: RefCell<Option<PlatformSnapshot>>,
}

impl PlatformSnapshotRepository for MemorySnapshotRepository {
    type Error = Infallible;

    fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error> {
        Ok(self.snapshot.borrow().clone())
    }

    fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error> {
        self.snapshot.replace(Some(snapshot.clone()));
        Ok(())
    }
}

fn notes_manifest() -> AppManifest {
    let mut runtime = RuntimeDescriptor::web();

    runtime
        .add_entrypoint(RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        ))
        .unwrap();

    AppManifest::new(
        AppId::parse("com.rumahl.notes").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Notes",
        runtime,
    )
    .unwrap()
}

#[test]
fn platform_state_survives_recovery_and_clean_uninstall() {
    let lifecycle = AppLifecycle::new();
    let persistence = PlatformPersistence::new();
    let repository = MemorySnapshotRepository::default();
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    let installed = lifecycle.install(notes_manifest(), &mut state).unwrap();
    let installation_id = *installed.installation_id();
    let user = UserIdentity::new(UserId::new());
    let mut policy = GrantIssuerPolicy::new();
    policy.set_user_role(*user.id(), UserRole::User);
    let grant = GrantAuthority::new()
        .issue(
            &policy,
            user.into(),
            installed.identity().clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![ResourceRef::new(
                ResourceNamespace::parse("rumahl.files").unwrap(),
                ResourceKind::parse("file").unwrap(),
                ResourceKey::parse("document-1").unwrap(),
            )],
        )
        .unwrap();
    let grant_id = *grant.id();
    grants.insert(grant);

    persistence.save(&state, &grants, &repository).unwrap();

    let mut recovered_state = PlatformState::new();
    let mut recovered_grants = InMemoryGrantStore::new();
    let report = persistence
        .load(&mut recovered_state, &mut recovered_grants, &repository)
        .unwrap()
        .unwrap();

    assert_eq!(report.installed_apps(), 1);
    assert_eq!(report.grants(), 1);
    assert!(
        recovered_state
            .installed_apps()
            .get_by_installation_id(&installation_id)
            .is_some()
    );
    assert!(recovered_grants.get(&grant_id).is_some());

    let uninstall = lifecycle
        .uninstall(
            &installation_id,
            &mut recovered_state,
            &mut recovered_grants,
        )
        .unwrap();

    assert_eq!(uninstall.revoked_grants().len(), 1);
    persistence
        .save(&recovered_state, &recovered_grants, &repository)
        .unwrap();

    let mut final_state = PlatformState::new();
    let mut final_grants = InMemoryGrantStore::new();
    let final_report = persistence
        .load(&mut final_state, &mut final_grants, &repository)
        .unwrap()
        .unwrap();

    assert_eq!(final_report.installed_apps(), 0);
    assert_eq!(final_report.grants(), 0);
    assert!(final_state.installed_apps().is_empty());
    assert!(final_grants.is_empty());
}
