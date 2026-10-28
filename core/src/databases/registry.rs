use std::error::Error;
use std::fmt;

use crate::AppIdentity;

use super::{AppDatabaseBinding, AppDatabaseId};

#[derive(Debug, Default, Clone)]
pub struct AppDatabaseRegistry {
    bindings: Vec<AppDatabaseBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppDatabaseRegistryError {
    AlreadyRegistered,
}

impl AppDatabaseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(
        &self,
        binding: &AppDatabaseBinding,
    ) -> Result<(), AppDatabaseRegistryError> {
        if self
            .bindings
            .iter()
            .any(|existing| existing.owner() == binding.owner() && existing.id() == binding.id())
        {
            return Err(AppDatabaseRegistryError::AlreadyRegistered);
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        binding: AppDatabaseBinding,
    ) -> Result<(), AppDatabaseRegistryError> {
        self.can_register(&binding)?;
        self.bindings.push(binding);

        Ok(())
    }

    pub fn database(
        &self,
        owner: &AppIdentity,
        database_id: &AppDatabaseId,
    ) -> Option<&AppDatabaseBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.owner() == owner && binding.id() == database_id)
    }

    pub fn databases_for_owner(&self, owner: &AppIdentity) -> Vec<&AppDatabaseBinding> {
        self.bindings
            .iter()
            .filter(|binding| binding.owner() == owner)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub(crate) fn remove_for_owner(&mut self, owner: &AppIdentity) -> usize {
        let before = self.bindings.len();
        self.bindings.retain(|binding| binding.owner() != owner);

        before - self.bindings.len()
    }
}

impl fmt::Display for AppDatabaseRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered => write!(f, "app database is already registered"),
        }
    }
}

impl Error for AppDatabaseRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppDatabaseDeclaration, AppId, InstallationId, PublisherId};

    fn app(app_id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(app_id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn binding(owner: AppIdentity, database_id: &str) -> AppDatabaseBinding {
        AppDatabaseBinding::new(
            owner,
            AppDatabaseDeclaration::new(AppDatabaseId::parse(database_id).unwrap()),
        )
    }

    #[test]
    fn resolves_database_by_installation_owner_and_logical_id() {
        let notes = app("com.rumahl.notes");
        let mut registry = AppDatabaseRegistry::new();
        registry
            .register(binding(notes.clone(), "primary"))
            .unwrap();

        assert!(
            registry
                .database(&notes, &AppDatabaseId::parse("primary").unwrap())
                .is_some()
        );
    }

    #[test]
    fn same_logical_id_is_isolated_between_installations() {
        let notes = app("com.rumahl.notes");
        let tasks = app("com.rumahl.tasks");
        let mut registry = AppDatabaseRegistry::new();

        registry.register(binding(notes, "primary")).unwrap();
        registry.register(binding(tasks, "primary")).unwrap();

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn rejects_duplicate_for_same_installation() {
        let notes = app("com.rumahl.notes");
        let database = binding(notes, "primary");
        let mut registry = AppDatabaseRegistry::new();

        registry.register(database.clone()).unwrap();
        assert_eq!(
            registry.register(database).unwrap_err(),
            AppDatabaseRegistryError::AlreadyRegistered
        );
    }

    #[test]
    fn removes_only_databases_for_requested_owner() {
        let notes = app("com.rumahl.notes");
        let tasks = app("com.rumahl.tasks");
        let mut registry = AppDatabaseRegistry::new();
        registry
            .register(binding(notes.clone(), "primary"))
            .unwrap();
        registry
            .register(binding(tasks.clone(), "primary"))
            .unwrap();

        assert_eq!(registry.remove_for_owner(&notes), 1);
        assert!(registry.databases_for_owner(&notes).is_empty());
        assert_eq!(registry.databases_for_owner(&tasks).len(), 1);
    }
}
