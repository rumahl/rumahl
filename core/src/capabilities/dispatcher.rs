use std::error::Error;
use std::fmt;

use crate::{AuthorizationDecision, AuthorizationEngine, PermissionGrant, ResourceRef};

use super::{
    CapabilityAccessRegistry, CapabilityAccessRuleError, CapabilityExecution, CapabilityInvocation,
    CapabilityRegistry,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct CapabilityDispatcher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDispatchError {
    ProviderNotRegistered,
    AccessRuleMissing,
    InvalidAccessRule(CapabilityAccessRuleError),
}

#[derive(Debug)]
pub enum CapabilityDispatchOutcome {
    Ready(Box<CapabilityExecution>),
    NotAuthorized(AuthorizationDecision),
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

    pub fn prepare_execution(
        &self,
        invocation: &CapabilityInvocation,
        resource: Option<ResourceRef>,
        capability_registry: &CapabilityRegistry,
        access_registry: &CapabilityAccessRegistry,
        authorization_engine: &AuthorizationEngine,
        grants: &[PermissionGrant],
    ) -> Result<CapabilityDispatchOutcome, CapabilityDispatchError> {
        let decision = self.authorize(
            invocation,
            resource.clone(),
            capability_registry,
            access_registry,
            authorization_engine,
            grants,
        )?;

        match decision {
            AuthorizationDecision::Allow => Ok(CapabilityDispatchOutcome::Ready(Box::new(
                CapabilityExecution::new(
                    invocation.context().clone(),
                    invocation.capability_provider().clone(),
                    resource,
                ),
            ))),

            decision => Ok(CapabilityDispatchOutcome::NotAuthorized(decision)),
        }
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
        OperationContext, PermissionId, PermissionScope, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace,
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

    #[test]
    fn prepares_execution_when_authorized() {
        let consumer = app("com.rumahl.notes");

        let provider_app = app("com.rumahl.files");

        let capability = preview_capability();

        let provider = CapabilityProvider::new(provider_app.into(), capability.clone()).unwrap();

        let invocation = CapabilityInvocation::new(
            OperationContext::for_background_app(consumer.clone()),
            provider.clone(),
        );

        let mut registry = CapabilityRegistry::new();

        registry.register(provider).unwrap();

        let mut access_registry = CapabilityAccessRegistry::new();

        let permission = PermissionId::parse("rumahl.files.read").unwrap();

        access_registry
            .register(CapabilityAccessRule::new(
                capability.clone(),
                permission.clone(),
            ))
            .unwrap();

        let resource = file("document-1");

        let grant = PermissionGrant::new(
            consumer.clone().into(),
            permission,
            PermissionScope::Explicit,
            vec![resource.clone()],
            consumer.into(),
        )
        .unwrap();

        let engine = AuthorizationEngine::new();

        let dispatcher = CapabilityDispatcher::new();

        let outcome = dispatcher
            .prepare_execution(
                &invocation,
                Some(resource.clone()),
                &registry,
                &access_registry,
                &engine,
                &[grant],
            )
            .unwrap();

        let CapabilityDispatchOutcome::Ready(execution) = outcome else {
            panic!("expected ready capability execution");
        };

        assert_eq!(execution.capability(), &capability);

        assert_eq!(execution.resource(), Some(&resource));

        assert_eq!(execution.provider(), invocation.capability_provider());
    }
}
