use rumahl_core::{
    AppId, AppIdentity, CapabilityId, ContributionError, ContributionId, InstallationId,
    PublisherId, SearchContribution, UserId, UserIdentity,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

#[test]
fn app_can_define_search_contribution() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let capability = CapabilityId::parse("com.rumahl.notes.search").unwrap();

    let search = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.clone().into(),
        capability.clone(),
    )
    .unwrap();

    assert_eq!(search.id().as_str(), "com.rumahl.notes.search");

    assert_eq!(search.kind().as_str(), "search-provider");

    assert_eq!(search.capability(), &capability);

    assert_eq!(search.owner(), &notes.into());
}

#[test]
fn different_apps_can_define_independent_search_contributions() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let notes_search = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.clone().into(),
        CapabilityId::parse("com.rumahl.notes.search").unwrap(),
    )
    .unwrap();

    let files_search = SearchContribution::new(
        ContributionId::parse("com.rumahl.files.search").unwrap(),
        files.clone().into(),
        CapabilityId::parse("com.rumahl.files.search").unwrap(),
    )
    .unwrap();

    assert_ne!(notes_search.id(), files_search.id());

    assert_ne!(notes_search.owner(), files_search.owner());

    assert_ne!(notes_search.capability(), files_search.capability());

    assert_eq!(notes_search.kind(), files_search.kind());
}

#[test]
fn search_contribution_keeps_capability_reference() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let capability = CapabilityId::parse("com.rumahl.notes.search").unwrap();

    let search = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.into(),
        capability.clone(),
    )
    .unwrap();

    assert_eq!(search.capability(), &capability);
}

#[test]
fn user_cannot_define_search_contribution() {
    let user = UserIdentity::new(UserId::new());

    let result = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        user.into(),
        CapabilityId::parse("com.rumahl.notes.search").unwrap(),
    );

    assert_eq!(result.unwrap_err(), ContributionError::UserCannotContribute);
}
