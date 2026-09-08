use rumahl_core::{
    AppId, AppIdentity, Contribution, ContributionId, ContributionKind, ContributionRegistry,
    InstallationId, PublisherId,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

fn contribution(id: &str, owner: AppIdentity, kind: &str) -> Contribution {
    Contribution::new(
        ContributionId::parse(id).unwrap(),
        owner.into(),
        ContributionKind::parse(kind).unwrap(),
    )
    .unwrap()
}

#[test]
fn multiple_apps_can_register_same_contribution_kind() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let notes_search = contribution("com.rumahl.notes.search", notes.clone(), "search-provider");

    let files_search = contribution("com.rumahl.files.search", files.clone(), "search-provider");

    let mut registry = ContributionRegistry::new();

    registry.register(notes_search).unwrap();

    registry.register(files_search).unwrap();

    let search_kind = ContributionKind::parse("search-provider").unwrap();

    let search_contributions = registry.contributions_for_kind(&search_kind);

    assert_eq!(search_contributions.len(), 2);

    let notes_identity = notes.into();

    let files_identity = files.into();

    assert!(
        search_contributions
            .iter()
            .any(|contribution| { contribution.owner() == &notes_identity })
    );

    assert!(
        search_contributions
            .iter()
            .any(|contribution| { contribution.owner() == &files_identity })
    );
}

#[test]
fn registry_can_find_all_contributions_for_owner() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let notes_identity = notes.clone().into();

    let mut registry = ContributionRegistry::new();

    registry
        .register(contribution(
            "com.rumahl.notes.new-note",
            notes.clone(),
            "command",
        ))
        .unwrap();

    registry
        .register(contribution("com.rumahl.notes.widget", notes, "widget"))
        .unwrap();

    registry
        .register(contribution(
            "com.rumahl.files.search",
            files,
            "search-provider",
        ))
        .unwrap();

    let notes_contributions = registry.contributions_for_owner(&notes_identity);

    assert_eq!(notes_contributions.len(), 2);

    assert!(
        notes_contributions
            .iter()
            .all(|contribution| { contribution.owner() == &notes_identity })
    );
}

#[test]
fn contribution_can_be_resolved_by_id() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let contribution_id = ContributionId::parse("com.rumahl.notes.new-note").unwrap();

    let command = Contribution::new(
        contribution_id.clone(),
        notes.into(),
        ContributionKind::parse("command").unwrap(),
    )
    .unwrap();

    let mut registry = ContributionRegistry::new();

    registry.register(command).unwrap();

    let resolved = registry.get(&contribution_id).unwrap();

    assert_eq!(resolved.id(), &contribution_id);

    assert_eq!(
        resolved.kind(),
        &ContributionKind::parse("command").unwrap()
    );
}
