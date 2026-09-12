use std::error::Error;
use std::fmt;

use crate::Identity;

use super::{Contribution, ContributionId, ContributionKind};

#[derive(Debug, Default)]
pub struct ContributionRegistry {
    contributions: Vec<Contribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionRegistryError {
    AlreadyRegistered,
}

impl ContributionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(
        &self,
        contribution: &Contribution,
    ) -> Result<(), ContributionRegistryError> {
        if self
            .contributions
            .iter()
            .any(|existing| existing.id() == contribution.id())
        {
            return Err(ContributionRegistryError::AlreadyRegistered);
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        contribution: Contribution,
    ) -> Result<(), ContributionRegistryError> {
        self.can_register(&contribution)?;

        self.contributions.push(contribution);

        Ok(())
    }

    pub fn get(&self, id: &ContributionId) -> Option<&Contribution> {
        self.contributions
            .iter()
            .find(|contribution| contribution.id() == id)
    }

    pub fn contributions_for_kind(&self, kind: &ContributionKind) -> Vec<&Contribution> {
        self.contributions
            .iter()
            .filter(|contribution| contribution.kind() == kind)
            .collect()
    }

    pub fn contributions_for_owner(&self, owner: &Identity) -> Vec<&Contribution> {
        self.contributions
            .iter()
            .filter(|contribution| contribution.owner() == owner)
            .collect()
    }

    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }

    pub fn len(&self) -> usize {
        self.contributions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }
}

impl fmt::Display for ContributionRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered => {
                write!(f, "contribution id is already registered")
            }
        }
    }
}

impl Error for ContributionRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, PublisherId};

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
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
    fn registers_contribution() {
        let mut registry = ContributionRegistry::new();

        registry
            .register(contribution(
                "com.rumahl.notes.new-note",
                app("com.rumahl.notes"),
                "command",
            ))
            .unwrap();

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn rejects_duplicate_contribution_id() {
        let notes = app("com.rumahl.notes");

        let first = contribution("com.rumahl.notes.new-note", notes.clone(), "command");

        let duplicate = contribution("com.rumahl.notes.new-note", notes, "widget");

        let mut registry = ContributionRegistry::new();

        registry.register(first).unwrap();

        assert_eq!(
            registry.register(duplicate).unwrap_err(),
            ContributionRegistryError::AlreadyRegistered
        );
    }

    //multi-app

    #[test]
    fn multiple_apps_can_contribute_same_kind() {
        let mut registry = ContributionRegistry::new();

        registry
            .register(contribution(
                "com.rumahl.notes.search",
                app("com.rumahl.notes"),
                "search-provider",
            ))
            .unwrap();

        registry
            .register(contribution(
                "com.rumahl.files.search",
                app("com.rumahl.files"),
                "search-provider",
            ))
            .unwrap();

        let kind = ContributionKind::parse("search-provider").unwrap();

        assert_eq!(registry.contributions_for_kind(&kind).len(), 2);
    }

    //owner filter
    #[test]
    fn lists_contributions_for_owner() {
        let notes = app("com.rumahl.notes");

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
                app("com.rumahl.files"),
                "search-provider",
            ))
            .unwrap();

        assert_eq!(registry.contributions_for_owner(&notes_identity).len(), 2);
    }

    #[test]
    fn can_register_new_contribution_without_mutating_registry() {
        let contribution = contribution(
            "com.rumahl.notes.new-note",
            app("com.rumahl.notes"),
            "command",
        );

        let registry = ContributionRegistry::new();

        assert!(registry.can_register(&contribution).is_ok());

        assert!(registry.is_empty());
    }

    #[test]
    fn cannot_register_existing_contribution() {
        let contribution = contribution(
            "com.rumahl.notes.new-note",
            app("com.rumahl.notes"),
            "command",
        );

        let mut registry = ContributionRegistry::new();

        registry.register(contribution.clone()).unwrap();

        assert_eq!(
            registry.can_register(&contribution).unwrap_err(),
            ContributionRegistryError::AlreadyRegistered
        );
    }
}
