use crate::{CapabilityId, Identity};

use super::{Contribution, ContributionError, ContributionId, ContributionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchContribution {
    contribution: Contribution,
    capability: CapabilityId,
}

impl SearchContribution {
    pub fn new(
        id: ContributionId,
        owner: Identity,
        capability: CapabilityId,
    ) -> Result<Self, ContributionError> {
        let kind = ContributionKind::parse("search-provider")
            .expect("search-provider is a valid contribution kind");

        let contribution = Contribution::new(id, owner, kind)?;

        Ok(Self {
            contribution,
            capability,
        })
    }

    pub fn contribution(&self) -> &Contribution {
        &self.contribution
    }

    pub fn id(&self) -> &ContributionId {
        self.contribution.id()
    }

    pub fn owner(&self) -> &Identity {
        self.contribution.owner()
    }

    pub fn kind(&self) -> &ContributionKind {
        self.contribution.kind()
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, PublisherId, UserId, UserIdentity};

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn app_can_define_search_contribution() {
        let capability = CapabilityId::parse("com.rumahl.notes.search").unwrap();

        let search = SearchContribution::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            notes_app().into(),
            capability.clone(),
        )
        .unwrap();

        assert_eq!(search.id().as_str(), "com.rumahl.notes.search");

        assert_eq!(search.kind().as_str(), "search-provider");

        assert_eq!(search.capability(), &capability);
    }

    #[test]
    fn search_contribution_preserves_owner() {
        let notes = notes_app();

        let notes_identity = notes.clone().into();

        let search = SearchContribution::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            notes.into(),
            CapabilityId::parse("com.rumahl.notes.search").unwrap(),
        )
        .unwrap();

        assert_eq!(search.owner(), &notes_identity);
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
}
