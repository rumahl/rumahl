use rumahl_core::{
    AppId, AppIdentity, CapabilityId, CommandAction, CommandContribution, CommandRegistry,
    ContributionId, InstallationId, PublisherId,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

fn capability_action(capability: &str) -> CommandAction {
    CommandAction::invoke_capability(CapabilityId::parse(capability).unwrap())
}

#[test]
fn commands_can_be_registered_and_discovered_across_apps() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let notes_identity = notes.clone().into();

    let files_identity = files.clone().into();

    let new_note = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
        notes.clone().into(),
        "New note",
        capability_action("com.rumahl.notes.create-note"),
    )
    .unwrap();

    let open_notes = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.open").unwrap(),
        notes.into(),
        "Open Notes",
        CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
    )
    .unwrap();

    let open_files = CommandContribution::new(
        ContributionId::parse("com.rumahl.files.open").unwrap(),
        files.into(),
        "Open Files",
        CommandAction::open_app(AppId::parse("com.rumahl.files").unwrap()),
    )
    .unwrap();

    let mut registry = CommandRegistry::new();

    registry.register(new_note).unwrap();

    registry.register(open_notes).unwrap();

    registry.register(open_files).unwrap();

    assert_eq!(registry.len(), 3);

    let notes_commands = registry.commands_for_owner(&notes_identity);

    assert_eq!(notes_commands.len(), 2);

    assert!(
        notes_commands
            .iter()
            .all(|command| { command.owner() == &notes_identity })
    );

    let files_commands = registry.commands_for_owner(&files_identity);

    assert_eq!(files_commands.len(), 1);

    assert_eq!(files_commands[0].title(), "Open Files");
}

#[test]
fn command_can_be_resolved_with_its_action() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let command_id = ContributionId::parse("com.rumahl.notes.new-note").unwrap();

    let capability = CapabilityId::parse("com.rumahl.notes.create-note").unwrap();

    let command = CommandContribution::new(
        command_id.clone(),
        notes.into(),
        "New note",
        CommandAction::invoke_capability(capability.clone()),
    )
    .unwrap();

    let mut registry = CommandRegistry::new();

    registry.register(command).unwrap();

    let resolved = registry.get(&command_id).unwrap();

    assert_eq!(resolved.id(), &command_id);

    assert_eq!(resolved.title(), "New note");

    assert_eq!(resolved.action().capability(), Some(&capability));

    assert_eq!(resolved.action().app_id(), None);
}

#[test]
fn open_app_action_survives_registry_round_trip() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let command_id = ContributionId::parse("com.rumahl.notes.open").unwrap();

    let app_id = AppId::parse("com.rumahl.notes").unwrap();

    let command = CommandContribution::new(
        command_id.clone(),
        notes.into(),
        "Open Notes",
        CommandAction::open_app(app_id.clone()),
    )
    .unwrap();

    let mut registry = CommandRegistry::new();

    registry.register(command).unwrap();

    let resolved = registry.get(&command_id).unwrap();

    assert_eq!(resolved.action().app_id(), Some(&app_id));

    assert_eq!(resolved.action().capability(), None);
}
