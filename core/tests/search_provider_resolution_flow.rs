use rumahl_core::{
    AppId, AppIdentity, CapabilityId, CapabilityProvider, CapabilityRegistry, ContributionId,
    InstallationId, PublisherId, SearchContribution, SearchProviderResolutionError,
    SearchProviderResolver,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

#[test]
fn search_contribution_resolves_its_own_provider() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let capability = CapabilityId::parse("rumahl.search.query").unwrap();

    let contribution = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.clone().into(),
        capability.clone(),
    )
    .unwrap();

    let notes_provider = CapabilityProvider::new(notes.clone().into(), capability.clone()).unwrap();

    let files_provider = CapabilityProvider::new(files.into(), capability.clone()).unwrap();

    let mut registry = CapabilityRegistry::new();

    /*
     * Register Files first on purpose.
     *
     * Resolution must not depend on
     * registration order.
     */
    registry.register(files_provider).unwrap();

    registry.register(notes_provider).unwrap();

    let resolver = SearchProviderResolver::new();

    let resolved = resolver.resolve(&contribution, &registry).unwrap();

    assert_eq!(resolved.capability(), &capability);

    assert_eq!(resolved.identity(), contribution.owner());

    assert_eq!(resolved.identity(), &notes.into());
}

#[test]
fn search_contribution_does_not_resolve_another_apps_provider() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let files = app("com.rumahl.files", "com.rumahl");

    let capability = CapabilityId::parse("rumahl.search.query").unwrap();

    let contribution = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.into(),
        capability.clone(),
    )
    .unwrap();

    /*
     * The capability exists, but only Files
     * is registered as its provider.
     */

    let files_provider = CapabilityProvider::new(files.into(), capability).unwrap();

    let mut registry = CapabilityRegistry::new();

    registry.register(files_provider).unwrap();

    let resolver = SearchProviderResolver::new();

    assert_eq!(
        resolver.resolve(&contribution, &registry,).unwrap_err(),
        SearchProviderResolutionError::ProviderNotRegistered
    );
}

#[test]
fn missing_search_provider_fails_closed() {
    let notes = app("com.rumahl.notes", "com.rumahl");

    let contribution = SearchContribution::new(
        ContributionId::parse("com.rumahl.notes.search").unwrap(),
        notes.into(),
        CapabilityId::parse("rumahl.search.query").unwrap(),
    )
    .unwrap();

    let registry = CapabilityRegistry::new();

    let resolver = SearchProviderResolver::new();

    assert_eq!(
        resolver.resolve(&contribution, &registry,).unwrap_err(),
        SearchProviderResolutionError::ProviderNotRegistered
    );
}
