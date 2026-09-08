use std::error::Error;
use std::fmt;

use crate::{AuthorizationDecision, AuthorizationEngine, PermissionGrant, ResourceRef};

use super::{
    CapabilityAccessRegistry, CapabilityAccessRuleError, CapabilityInvocation, CapabilityRegistry,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct CapabilityDispatcher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDispatchError {
    ProviderNotRegistered,
    AccessRuleMissing,
    InvalidAccessRule(CapabilityAccessRuleError),
}

impl CapabilityDispatcher {
    pub fn new() -> Self {
        Self
    }

    pub fn authorize(
        &self,
        invocation: &CapabilityInvocation,
        resource: Option<ResourceRef>,
        capability_registry: &CapabilityRegistry,
        access_registry: &CapabilityAccessRegistry,
        authorization_engine: &AuthorizationEngine,
        grants: &[PermissionGrant],
    ) -> Result<AuthorizationDecision, CapabilityDispatchError> {
        if !capability_registry.is_registered(invocation.capability_provider()) {
            return Err(CapabilityDispatchError::ProviderNotRegistered);
        }

        let access_rule = access_registry
            .rule_for(invocation.capability())
            .ok_or(CapabilityDispatchError::AccessRuleMissing)?;

        let authorization_request = access_rule
            .authorization_request(invocation, resource)
            .map_err(CapabilityDispatchError::InvalidAccessRule)?;

        Ok(authorization_engine.authorize(&authorization_request, grants))
    }
}

impl fmt::Display for CapabilityDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderNotRegistered => {
                write!(f, "capability provider is not registered")
            }

            Self::AccessRuleMissing => {
                write!(f, "capability has no registered access rule")
            }

            Self::InvalidAccessRule(error) => {
                write!(
                    f,
                    "capability access rule is invalid for invocation: {error}"
                )
            }
        }
    }
}

impl Error for CapabilityDispatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAccessRule(error) => Some(error),

            Self::ProviderNotRegistered | Self::AccessRuleMissing => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, CapabilityAccessRule, CapabilityId, CapabilityProvider, InstallationId,
        OperationContext, PermissionId, PublisherId, ResourceKey, ResourceKind, ResourceNamespace,
    };

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn file(key: &str) -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse(key).unwrap(),
        )
    }

    fn preview_capability() -> CapabilityId {
        CapabilityId::parse("rumahl.files.preview").unwrap()
    }

    fn preview_invocation(consumer: AppIdentity, provider: AppIdentity) -> CapabilityInvocation {
        let capability = preview_capability();

        let provider = CapabilityProvider::new(provider.into(), capability).unwrap();

        CapabilityInvocation::new(OperationContext::for_background_app(consumer), provider)
    }

    #[test]
    fn rejects_unregistered_provider() {
        let invocation = preview_invocation(app("com.rumahl.notes"), app("com.rumahl.files"));

        let registry = CapabilityRegistry::new();

        let access_registry = CapabilityAccessRegistry::new();

        let engine = AuthorizationEngine::new();

        let dispatcher = CapabilityDispatcher::new();

        let result = dispatcher.authorize(
            &invocation,
            Some(file("document-1")),
            &registry,
            &access_registry,
            &engine,
            &[],
        );

        assert_eq!(
            result.unwrap_err(),
            CapabilityDispatchError::ProviderNotRegistered
        );
    }

    #[test]
    fn rejects_capability_without_access_rule() {
        let consumer = app("com.rumahl.notes");

        let provider_app = app("com.rumahl.files");

        let capability = preview_capability();

        let provider = CapabilityProvider::new(provider_app.into(), capability).unwrap();

        let invocation = CapabilityInvocation::new(
            OperationContext::for_background_app(consumer),
            provider.clone(),
        );

        let mut registry = CapabilityRegistry::new();

        registry.register(provider).unwrap();

        let access_registry = CapabilityAccessRegistry::new();

        let engine = AuthorizationEngine::new();

        let dispatcher = CapabilityDispatcher::new();

        let result = dispatcher.authorize(
            &invocation,
            Some(file("document-1")),
            &registry,
            &access_registry,
            &engine,
            &[],
        );

        assert_eq!(
            result.unwrap_err(),
            CapabilityDispatchError::AccessRuleMissing
        );
    }

    #[test]
    fn returns_authorization_denial() {
        let consumer = app("com.rumahl.notes");

        let provider_app = app("com.rumahl.files");

        let capability = preview_capability();

        let provider = CapabilityProvider::new(provider_app.into(), capability.clone()).unwrap();

        let invocation = CapabilityInvocation::new(
            OperationContext::for_background_app(consumer),
            provider.clone(),
        );

        let mut registry = CapabilityRegistry::new();

        registry.register(provider).unwrap();

        let mut access_registry = CapabilityAccessRegistry::new();

        access_registry
            .register(CapabilityAccessRule::new(
                capability,
                PermissionId::parse("rumahl.files.read").unwrap(),
            ))
            .unwrap();

        let engine = AuthorizationEngine::new();

        let dispatcher = CapabilityDispatcher::new();

        let result = dispatcher
            .authorize(
                &invocation,
                Some(file("document-1")),
                &registry,
                &access_registry,
                &engine,
                &[],
            )
            .unwrap();

        assert_eq!(
            result,
            AuthorizationDecision::Deny(crate::AuthorizationDenyReason::NoGrantForActor)
        );
    }
}
