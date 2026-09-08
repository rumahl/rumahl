use std::error::Error;
use std::fmt;

use super::{
    CapabilityId,
    CapabilityProvider,
};

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    providers: Vec<CapabilityProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityRegistryError {
    AlreadyRegistered,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        provider: CapabilityProvider,
    ) -> Result<(), CapabilityRegistryError> {
        if self.providers.contains(&provider) {
            return Err(
                CapabilityRegistryError::AlreadyRegistered
            );
        }

        self.providers.push(provider);

        Ok(())
    }

    pub fn providers_for(
        &self,
        capability: &CapabilityId,
    ) -> Vec<&CapabilityProvider> {
        self.providers
            .iter()
            .filter(|provider| {
                provider.capability() == capability
            })
            .collect()
    }

    pub fn is_registered(
        &self,
        provider: &CapabilityProvider,
    ) -> bool {
        self.providers.contains(provider)
    }

    pub fn providers(&self) -> &[CapabilityProvider] {
        &self.providers
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl fmt::Display for CapabilityRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered => {
                write!(
                    f,
                    "capability provider is already registered"
                )
            }
        }
    }
}

impl Error for CapabilityRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId,
        AppIdentity,
        InstallationId,
        PublisherId,
    };

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn search_provider(
        app: AppIdentity,
    ) -> CapabilityProvider {
        CapabilityProvider::new(
            app.into(),
            CapabilityId::parse(
                "rumahl.search.query"
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn registers_provider() {
        let provider =
            search_provider(
                app("com.rumahl.notes")
            );

        let mut registry =
            CapabilityRegistry::new();

        registry
            .register(provider)
            .unwrap();

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn rejects_duplicate_registration() {
        let provider =
            search_provider(
                app("com.rumahl.notes")
            );

        let mut registry =
            CapabilityRegistry::new();

        registry
            .register(provider.clone())
            .unwrap();

        assert_eq!(
            registry.register(provider).unwrap_err(),
            CapabilityRegistryError::AlreadyRegistered
        );
    }

        #[test]
    fn capability_can_have_multiple_providers() {
        let notes =
            search_provider(
                app("com.rumahl.notes")
            );

        let files =
            search_provider(
                app("com.rumahl.files")
            );

        let capability =
            CapabilityId::parse(
                "rumahl.search.query"
            )
            .unwrap();

        let mut registry =
            CapabilityRegistry::new();

        registry.register(notes).unwrap();
        registry.register(files).unwrap();

        let providers =
            registry.providers_for(&capability);

        assert_eq!(providers.len(), 2);
    }

        #[test]
    fn providers_are_filtered_by_capability() {
        let notes =
            search_provider(
                app("com.rumahl.notes")
            );

        let preview_provider =
            CapabilityProvider::new(
                app("com.rumahl.files").into(),
                CapabilityId::parse(
                    "rumahl.files.preview"
                )
                .unwrap(),
            )
            .unwrap();

        let mut registry =
            CapabilityRegistry::new();

        registry.register(notes).unwrap();

        registry
            .register(preview_provider)
            .unwrap();

        let search =
            CapabilityId::parse(
                "rumahl.search.query"
            )
            .unwrap();

        assert_eq!(
            registry.providers_for(&search).len(),
            1
        );
    }
}