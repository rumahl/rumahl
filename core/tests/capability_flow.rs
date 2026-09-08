use rumahl_core::{
    AppId,
    AppIdentity,
    CapabilityId,
    CapabilityProvider,
    CapabilityRegistry,
    InstallationId,
    PublisherId,
};

fn app(
    app_id: &str,
    publisher_id: &str,
) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

#[test]
fn multiple_apps_can_provide_same_capability() {
    let notes = app(
        "com.rumahl.notes",
        "com.rumahl",
    );

    let files = app(
        "com.rumahl.files",
        "com.rumahl",
    );

    let search =
        CapabilityId::parse(
            "rumahl.search.query"
        )
        .unwrap();

    let notes_provider =
        CapabilityProvider::new(
            notes.clone().into(),
            search.clone(),
        )
        .unwrap();

    let files_provider =
        CapabilityProvider::new(
            files.clone().into(),
            search.clone(),
        )
        .unwrap();

    let mut registry =
        CapabilityRegistry::new();

    registry
        .register(notes_provider)
        .unwrap();

    registry
        .register(files_provider)
        .unwrap();

    let providers =
        registry.providers_for(&search);

    assert_eq!(providers.len(), 2);

    assert!(
        providers
            .iter()
            .any(|provider| {
                provider.identity()
                    == &notes.clone().into()
            })
    );

    assert!(
        providers
            .iter()
            .any(|provider| {
                provider.identity()
                    == &files.clone().into()
            })
    );
}

#[test]
fn registry_returns_only_matching_capability_providers() {
    let notes = app(
        "com.rumahl.notes",
        "com.rumahl",
    );

    let files = app(
        "com.rumahl.files",
        "com.rumahl",
    );

    let search =
        CapabilityId::parse(
            "rumahl.search.query"
        )
        .unwrap();

    let preview =
        CapabilityId::parse(
            "rumahl.files.preview"
        )
        .unwrap();

    let search_provider =
        CapabilityProvider::new(
            notes.clone().into(),
            search.clone(),
        )
        .unwrap();

    let preview_provider =
        CapabilityProvider::new(
            files.into(),
            preview.clone(),
        )
        .unwrap();

    let mut registry =
        CapabilityRegistry::new();

    registry
        .register(search_provider)
        .unwrap();

    registry
        .register(preview_provider)
        .unwrap();

    let search_providers =
        registry.providers_for(&search);

    assert_eq!(
        search_providers.len(),
        1
    );

    assert_eq!(
        search_providers[0].identity(),
        &notes.into()
    );

    let preview_providers =
        registry.providers_for(&preview);

    assert_eq!(
        preview_providers.len(),
        1
    );
}