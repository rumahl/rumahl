use rumahl_core::{
    AppId, AppIdentity, CapabilityId, CommandAction, CommandContribution, CommandContributionError,
    ContributionError, ContributionId, InstallationId, PublisherId, UserId, UserIdentity,
};

fn create_note_action() -> CommandAction {
    CommandAction::invoke_capability(CapabilityId::parse("com.rumahl.notes.create-note").unwrap())
}

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

#[test]
fn app_can_define_command_contribution() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let command = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
        notes.clone().into(),
        "New note",
        create_note_action(),
    )
    .unwrap();

    assert_eq!(command.id().as_str(), "com.rumahl.notes.new-note");

    assert_eq!(command.title(), "New note");

    assert_eq!(command.kind().as_str(), "command");

    assert_eq!(command.owner(), &notes.into());
}

#[test]
fn command_title_is_normalized() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let command = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
        notes.into(),
        "   New note   ",
        create_note_action(),
    )
    .unwrap();

    assert_eq!(command.title(), "New note");
}

#[test]
fn user_cannot_define_command_contribution() {
    let user = UserIdentity::new(UserId::new());

    let result = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
        user.into(),
        "New note",
        create_note_action(),
    );

    assert_eq!(
        result.unwrap_err(),
        CommandContributionError::InvalidContribution(ContributionError::UserCannotContribute)
    );
}

#[test]
fn command_kind_is_always_command() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let command = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.open").unwrap(),
        notes.into(),
        "Open Notes",
        CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
    )
    .unwrap();

    assert_eq!(command.kind().as_str(), "command");
}

#[test]
fn command_can_reference_capability_action() {
    let capability = CapabilityId::parse("com.rumahl.notes.create-note").unwrap();
    let command = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
        app("com.rumahl.notes", "com.rumahl").into(),
        "New note",
        CommandAction::invoke_capability(capability.clone()),
    )
    .unwrap();
    assert_eq!(command.action().capability(), Some(&capability));
    assert_eq!(command.action().app_id(), None);
}

#[test]
fn command_can_reference_app_action() {
    let app_id = AppId::parse("com.rumahl.notes").unwrap();
    let command = CommandContribution::new(
        ContributionId::parse("com.rumahl.notes.open").unwrap(),
        app("com.rumahl.notes", "com.rumahl").into(),
        "Open Notes",
        CommandAction::open_app(app_id.clone()),
    )
    .unwrap();
    assert_eq!(command.action().app_id(), Some(&app_id));
    assert_eq!(command.action().capability(), None);
}
