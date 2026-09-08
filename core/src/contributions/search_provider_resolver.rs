use std::error::Error;
use std::fmt;

use crate::{CapabilityProvider, CapabilityRegistry};

use super::SearchContribution;

#[derive(Debug, Default, Clone, Copy)]
pub struct SearchProviderResolver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchProviderResolutionError {
    ProviderNotRegistered,
}

impl SearchProviderResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve<'a>(
        &self,
        contribution: &SearchContribution,
        capability_registry: &'a CapabilityRegistry,
    ) -> Result<&'a CapabilityProvider, SearchProviderResolutionError> {
        capability_registry
            .provider(contribution.capability(), contribution.owner())
            .ok_or(SearchProviderResolutionError::ProviderNotRegistered)
    }
}

impl fmt::Display for SearchProviderResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderNotRegistered => {
                write!(
                    f,
                    "search contribution provider is not registered for its capability"
                )
            }
        }
    }
}

impl Error for SearchProviderResolutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, CapabilityId, CapabilityProvider, CapabilityRegistry, ContributionId,
        InstallationId, PublisherId, SearchContribution,
    };

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn resolves_provider_owned_by_search_contribution() {
        let notes = app("com.rumahl.notes");

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        let contribution = SearchContribution::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            notes.clone().into(),
            capability.clone(),
        )
        .unwrap();

        let provider = CapabilityProvider::new(notes.clone().into(), capability.clone()).unwrap();

        let mut registry = CapabilityRegistry::new();

        registry.register(provider).unwrap();

        let resolver = SearchProviderResolver::new();

        let resolved = resolver.resolve(&contribution, &registry).unwrap();

        assert_eq!(resolved.identity(), &notes.into());

        assert_eq!(resolved.capability(), &capability);
    }

    #[test]
    fn resolves_correct_provider_when_capability_has_multiple_providers() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        let contribution = SearchContribution::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            notes.clone().into(),
            capability.clone(),
        )
        .unwrap();

        let notes_provider =
            CapabilityProvider::new(notes.clone().into(), capability.clone()).unwrap();

        let files_provider = CapabilityProvider::new(files.into(), capability).unwrap();

        let mut registry = CapabilityRegistry::new();

        registry.register(files_provider).unwrap();

        registry.register(notes_provider).unwrap();

        let resolver = SearchProviderResolver::new();

        let resolved = resolver.resolve(&contribution, &registry).unwrap();

        assert_eq!(resolved.identity(), &notes.into());
    }

    #[test]
    fn rejects_registered_capability_owned_by_different_provider() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        let contribution = SearchContribution::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            notes.into(),
            capability.clone(),
        )
        .unwrap();

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
    fn rejects_missing_provider() {
        let notes = app("com.rumahl.notes");

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
}
