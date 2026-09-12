use rumahl_core::{
    AppId, AppLifecycle, AppLifecycleError, AppManifest, AppVersion, CapabilityId, CommandAction,
    CommandContributionDeclaration, ContributionId, EventName, Identity, InstalledAppRegistryError,
    PermissionId, PermissionRequest, PermissionScope, PlatformState, PublisherId,
    SearchContributionDeclaration,
};

fn notes_manifest() -> AppManifest {
    let mut manifest = AppManifest::new(
        AppId::parse("com.rumahl.notes").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Notes",
    )
    .unwrap();

    manifest
        .add_permission_request(PermissionRequest::new(
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::UserSelected,
            true,
            Some("Open files selected by the user".to_owned()),
        ))
        .unwrap();

    manifest
        .add_provided_capability(CapabilityId::parse("rumahl.search.query").unwrap())
        .unwrap();

    manifest
        .add_provided_capability(CapabilityId::parse("com.rumahl.notes.create-note").unwrap())
        .unwrap();

    manifest
        .add_contribution(
            CommandContributionDeclaration::new(
                ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
                "New note",
                CommandAction::invoke_capability(
                    CapabilityId::parse("com.rumahl.notes.create-note").unwrap(),
                ),
            )
            .unwrap()
            .into(),
        )
        .unwrap();

    manifest
        .add_contribution(
            SearchContributionDeclaration::new(
                ContributionId::parse("com.rumahl.notes.search").unwrap(),
                CapabilityId::parse("rumahl.search.query").unwrap(),
            )
            .into(),
        )
        .unwrap();

    manifest
        .add_event_subscription(EventName::parse("rumahl.files.changed").unwrap())
        .unwrap();

    manifest
}

#[test]
fn complete_app_lifecycle_flow() {
    let lifecycle = AppLifecycle::new();

    let mut state = PlatformState::new();

    /*
     * Install.
     */

    let installed = lifecycle.install(notes_manifest(), &mut state).unwrap();

    let app_id = AppId::parse("com.rumahl.notes").unwrap();

    assert_eq!(installed.identity().app_id(), &app_id);

    assert_eq!(installed.manifest().version(), &AppVersion::new(1, 0, 0,));

    /*
     * Installed app state.
     */

    assert_eq!(state.installed_apps().len(), 1);

    let stored = state.installed_apps().get_by_app_id(&app_id).unwrap();

    assert_eq!(stored.installation_id(), installed.installation_id());

    /*
     * Manifest declarations remain available
     * through the installed app.
     */

    assert_eq!(stored.manifest().permission_requests().len(), 1);

    assert_eq!(
        stored.manifest().permission_requests()[0]
            .permission()
            .as_str(),
        "rumahl.files.read"
    );

    assert_eq!(stored.manifest().provided_capabilities().len(), 2);

    assert_eq!(stored.manifest().contributions().len(), 2);

    assert_eq!(stored.manifest().event_subscriptions().len(), 1);

    /*
     * Concrete platform registration.
     */

    let identity: Identity = installed.identity().clone().into();

    let search_capability = CapabilityId::parse("rumahl.search.query").unwrap();

    let create_note_capability = CapabilityId::parse("com.rumahl.notes.create-note").unwrap();

    assert!(
        state
            .capability_registry()
            .provider(&search_capability, &identity,)
            .is_some()
    );

    assert!(
        state
            .capability_registry()
            .provider(&create_note_capability, &identity,)
            .is_some()
    );

    /*
     * Base contributions.
     */

    let command_id = ContributionId::parse("com.rumahl.notes.new-note").unwrap();

    let search_id = ContributionId::parse("com.rumahl.notes.search").unwrap();

    assert!(state.contribution_registry().get(&command_id).is_some());

    assert!(state.contribution_registry().get(&search_id).is_some());

    /*
     * Specialized command registration.
     */

    let command = state.command_registry().get(&command_id).unwrap();

    assert_eq!(command.owner(), &identity);

    assert_eq!(command.title(), "New note");

    assert_eq!(command.action().capability(), Some(&create_note_capability));

    /*
     * Specialized search registration.
     */

    let search = state.search_registry().get(&search_id).unwrap();

    assert_eq!(search.owner(), &identity);

    assert_eq!(search.capability(), &search_capability);

    /*
     * Event subscription.
     */

    let event = EventName::parse("rumahl.files.changed").unwrap();

    assert!(
        state
            .event_bus()
            .subscriptions()
            .iter()
            .any(|subscription| {
                subscription.subscriber() == &identity && subscription.event() == &event
            })
    );

    /*
     * Installing the same AppId again must
     * fail without modifying the platform.
     */

    let result = lifecycle.install(notes_manifest(), &mut state);

    assert_eq!(
        result.unwrap_err(),
        AppLifecycleError::InstalledAppConflict(InstalledAppRegistryError::AppAlreadyInstalled)
    );

    assert_eq!(state.installed_apps().len(), 1);

    assert_eq!(state.capability_registry().len(), 2);

    assert_eq!(state.contribution_registry().len(), 2);

    assert_eq!(state.command_registry().len(), 1);

    assert_eq!(state.search_registry().len(), 1);

    assert_eq!(state.event_bus().len(), 1);

    /*
     * Uninstall.
     */

    let installation_id = *installed.installation_id();

    let result = lifecycle.uninstall(&installation_id, &mut state).unwrap();

    assert_eq!(result.app().installation_id(), &installation_id);

    assert_eq!(result.deregistration().capability_providers(), 2);

    assert_eq!(result.deregistration().contributions(), 2);

    assert_eq!(result.deregistration().commands(), 1);

    assert_eq!(result.deregistration().searches(), 1);

    assert_eq!(result.deregistration().event_subscriptions(), 1);

    /*
     * Entire platform state is clean again.
     */

    assert!(state.installed_apps().is_empty());

    assert!(state.capability_registry().is_empty());

    assert!(state.contribution_registry().is_empty());

    assert!(state.command_registry().is_empty());

    assert!(state.search_registry().is_empty());

    assert!(state.event_bus().is_empty());

    /*
     * Public uninstall semantics:
     * unknown installation is an error.
     */

    assert!(matches!(
        lifecycle.uninstall(&installation_id, &mut state,),
        Err(AppLifecycleError::InstallationNotFound)
    ));
}
