use rumahl_core::{
    AppId, AppIdentity, CommandContribution, CommandContributionError, ContributionError,
    ContributionId, InstallationId, PublisherId, UserId, UserIdentity,
};

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
    )
    .unwrap();

    assert_eq!(command.kind().as_str(), "command");
}
