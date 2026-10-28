use std::error::Error;
use std::fmt;

use crate::Identity;

use super::{ContributionId, ContributionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    id: ContributionId,
    owner: Identity,
    kind: ContributionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionError {
    UserCannotContribute,
}

impl Contribution {
    pub fn new(
        id: ContributionId,
        owner: Identity,
        kind: ContributionKind,
    ) -> Result<Self, ContributionError> {
        if owner.is_user() {
            return Err(ContributionError::UserCannotContribute);
        }

        Ok(Self { id, owner, kind })
    }

    pub fn id(&self) -> &ContributionId {
        &self.id
    }

    pub fn owner(&self) -> &Identity {
        &self.owner
    }

    pub fn kind(&self) -> &ContributionKind {
        &self.kind
    }
}

impl fmt::Display for ContributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserCannotContribute => {
                write!(f, "users cannot register platform contributions")
            }
        }
    }
}

impl Error for ContributionError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, PublisherId, UserId, UserIdentity};

    #[test]
    fn app_can_create_contribution() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let result = Contribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            app.into(),
            ContributionKind::parse("command").unwrap(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn user_cannot_create_contribution() {
        let user = UserIdentity::new(UserId::new());

        let result = Contribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            user.into(),
            ContributionKind::parse("command").unwrap(),
        );

        assert_eq!(result.unwrap_err(), ContributionError::UserCannotContribute);
    }
}
