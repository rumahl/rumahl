use std::error::Error;
use std::fmt;

use crate::Identity;

use super::{ContributionId, SearchContribution};

#[derive(Debug, Default)]
pub struct SearchRegistry {
    providers: Vec<SearchContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchRegistryError {
    AlreadyRegistered,
}

impl SearchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(
        &self,
        contribution: &SearchContribution,
    ) -> Result<(), SearchRegistryError> {
        if self
            .providers
            .iter()
            .any(|existing| existing.id() == contribution.id())
        {
            return Err(SearchRegistryError::AlreadyRegistered);
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        contribution: SearchContribution,
    ) -> Result<(), SearchRegistryError> {
        self.can_register(&contribution)?;

        self.providers.push(contribution);

        Ok(())
    }

    pub fn get(&self, id: &ContributionId) -> Option<&SearchContribution> {
        self.providers
            .iter()
            .find(|contribution| contribution.id() == id)
    }

    pub fn providers_for_owner(&self, owner: &Identity) -> Vec<&SearchContribution> {
        self.providers
            .iter()
            .filter(|contribution| contribution.owner() == owner)
            .collect()
    }

    pub fn providers(&self) -> &[SearchContribution] {
        &self.providers
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl fmt::Display for SearchRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered => {
                write!(f, "search contribution is already registered")
            }
        }
    }
}

impl Error for SearchRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, CapabilityId, InstallationId, PublisherId};

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
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
    fn registers_search_contribution() {
        let mut registry = SearchRegistry::new();

        registry
            .register(search(
                "com.rumahl.notes.search",
                app("com.rumahl.notes"),
                "com.rumahl.notes.search",
            ))
            .unwrap();

        assert_eq!(registry.len(), 1);

        assert!(!registry.is_empty());
    }

    #[test]
    fn resolves_search_contribution_by_id() {
        let id = ContributionId::parse("com.rumahl.notes.search").unwrap();

        let mut registry = SearchRegistry::new();

        registry
            .register(search(
                id.as_str(),
                app("com.rumahl.notes"),
                "com.rumahl.notes.search",
            ))
            .unwrap();

        let resolved = registry.get(&id).unwrap();

        assert_eq!(resolved.id(), &id);

        assert_eq!(resolved.capability().as_str(), "com.rumahl.notes.search");
    }

    #[test]
    fn rejects_duplicate_search_contribution_id() {
        let notes = app("com.rumahl.notes");

        let first = search(
            "com.rumahl.notes.search",
            notes.clone(),
            "com.rumahl.notes.search",
        );

        let duplicate = search(
            "com.rumahl.notes.search",
            notes,
            "com.rumahl.notes.search-v2",
        );

        let mut registry = SearchRegistry::new();

        registry.register(first).unwrap();

        assert_eq!(
            registry.register(duplicate).unwrap_err(),
            SearchRegistryError::AlreadyRegistered
        );
    }

    #[test]
    fn lists_search_providers_for_owner() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

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

        registry
            .register(search(
                "com.rumahl.files.search",
                files,
                "com.rumahl.files.search",
            ))
            .unwrap();

        let providers = registry.providers_for_owner(&notes_identity);

        assert_eq!(providers.len(), 2);

        assert!(
            providers
                .iter()
                .all(|contribution| { contribution.owner() == &notes_identity })
        );
    }

    #[test]
    fn multiple_apps_can_register_search_providers() {
        let mut registry = SearchRegistry::new();

        registry
            .register(search(
                "com.rumahl.notes.search",
                app("com.rumahl.notes"),
                "com.rumahl.notes.search",
            ))
            .unwrap();

        registry
            .register(search(
                "com.rumahl.files.search",
                app("com.rumahl.files"),
                "com.rumahl.files.search",
            ))
            .unwrap();

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn can_register_new_search_provider_without_mutating_registry() {
        let contribution = search(
            "com.rumahl.notes.search",
            app("com.rumahl.notes"),
            "com.rumahl.notes.search",
        );

        let registry = SearchRegistry::new();

        assert!(registry.can_register(&contribution).is_ok());

        assert!(registry.is_empty());
    }

    #[test]
    fn cannot_register_existing_search_provider() {
        let contribution = search(
            "com.rumahl.notes.search",
            app("com.rumahl.notes"),
            "com.rumahl.notes.search",
        );

        let mut registry = SearchRegistry::new();

        registry.register(contribution.clone()).unwrap();

        assert_eq!(
            registry.can_register(&contribution).unwrap_err(),
            SearchRegistryError::AlreadyRegistered
        );
    }
}
