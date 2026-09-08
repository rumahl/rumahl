use std::error::Error;
use std::fmt;

use crate::identity::Identity;

use super::CapabilityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProvider {
    identity: Identity,
    capability: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityProviderError {
    UserCannotProvideCapability,
}

impl CapabilityProvider {
    pub fn new(
        identity: Identity,
        capability: CapabilityId,
    ) -> Result<Self, CapabilityProviderError> {
        if identity.is_user() {
            return Err(CapabilityProviderError::UserCannotProvideCapability);
        }

        Ok(Self {
            identity,
            capability,
        })
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }
}

impl fmt::Display for CapabilityProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserCannotProvideCapability => {
                write!(f, "users cannot provide platform capabilities")
            }
        }
    }
}

impl Error for CapabilityProviderError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, InstallationId, PublisherId, ServiceId, ServiceIdentity, UserId,
        UserIdentity,
    };

    #[test]
    fn app_can_provide_capability() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let provider = CapabilityProvider::new(
            app.into(),
            CapabilityId::parse("rumahl.search.query").unwrap(),
        );

        assert!(provider.is_ok());
    }

    #[test]
    fn service_can_provide_capability() {
        let service = ServiceIdentity::new(ServiceId::parse("rumahl.search-service").unwrap());

        let provider = CapabilityProvider::new(
            service.into(),
            CapabilityId::parse("rumahl.search.query").unwrap(),
        );

        assert!(provider.is_ok());
    }

    #[test]
    fn user_cannot_provide_capability() {
        let user = UserIdentity::new(UserId::new());

        let result = CapabilityProvider::new(
            user.into(),
            CapabilityId::parse("rumahl.search.query").unwrap(),
        );

        assert_eq!(
            result.unwrap_err(),
            CapabilityProviderError::UserCannotProvideCapability
        );
    }
}
