use rumahl_core::{
    AppId, AppIdentity, CapabilityId, ContributionId, InstallationId, PublisherId,
    SearchContribution, SearchRegistry,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

fn search(id: &str, owner: AppIdentity, capability: &str) -> SearchContribution {
    SearchContribution::new(
        ContributionId::parse(id).unwrap(),
        owner.into(),
        CapabilityId::parse(capability).unwrap(),
    )
    .unwrap()
}

#[test]
fn search_contributions_can_be_registered_across_apps() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let notes_identity = notes.clone().into();

    let files_identity = files.clone().into();

    let mut registry = SearchRegistry::new();

    registry
        .register(search(
            "com.rumahl.notes.search",
            notes,
            "com.rumahl.notes.search",
        ))
        .unwrap();

    registry
        .register(search(
            "com.rumahl.files.search",
            files,
            "com.rumahl.files.search",
        ))
        .unwrap();

    assert_eq!(registry.len(), 2);

    let notes_contributions = registry.contributions_for_owner(&notes_identity);

    assert_eq!(notes_contributions.len(), 1);

    assert_eq!(
        notes_contributions[0].capability().as_str(),
        "com.rumahl.notes.search"
    );

    let files_contributions = registry.contributions_for_owner(&files_identity);

    assert_eq!(files_contributions.len(), 1);

    assert_eq!(
        files_contributions[0].capability().as_str(),
        "com.rumahl.files.search"
    );
}

#[test]
fn search_contribution_can_be_resolved_with_capability() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let contribution_id = ContributionId::parse("com.rumahl.notes.search").unwrap();

    let capability = CapabilityId::parse("com.rumahl.notes.search").unwrap();

    let contribution =
        SearchContribution::new(contribution_id.clone(), notes.into(), capability.clone()).unwrap();

    let mut registry = SearchRegistry::new();

    registry.register(contribution).unwrap();

    let resolved = registry.get(&contribution_id).unwrap();

    assert_eq!(resolved.id(), &contribution_id);

    assert_eq!(resolved.capability(), &capability);

    assert_eq!(resolved.kind().as_str(), "search-provider");
}

#[test]
fn owner_can_register_multiple_search_contributions() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let notes_identity = notes.clone().into();

    let mut registry = SearchRegistry::new();

    registry
        .register(search(
            "com.rumahl.notes.search",
            notes.clone(),
            "com.rumahl.notes.search",
        ))
        .unwrap();

    registry
        .register(search(
            "com.rumahl.notes.search-recent",
            notes,
            "com.rumahl.notes.search-recent",
        ))
        .unwrap();

    let contributions = registry.contributions_for_owner(&notes_identity);

    assert_eq!(contributions.len(), 2);

    assert!(
        contributions
            .iter()
            .all(|contribution| { contribution.owner() == &notes_identity })
    );
}
