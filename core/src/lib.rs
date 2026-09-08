pub mod capabilities;
pub mod context;
pub mod contributions;
pub mod identity;
pub mod permissions;
pub mod resources;

pub use context::{CorrelationId, CorrelationIdError, OperationContext};

pub use identity::{
    AppId, AppIdError, AppIdentity, Identity, InstallationId, InstallationIdError, PublisherId,
    PublisherIdError, ServiceId, ServiceIdError, ServiceIdentity, SessionId, SessionIdError,
    UserId, UserIdError, UserIdentity,
};

pub use resources::{
    ResourceKey, ResourceKeyError, ResourceKind, ResourceKindError, ResourceNamespace,
    ResourceNamespaceError, ResourceRef,
};

pub use permissions::{
    AuthorizationDecision, AuthorizationDenyReason, AuthorizationEngine, AuthorizationRequest,
    GrantAuthority, GrantAuthorityError, GrantId, GrantIssuerPolicy, GrantIssuerPolicyError,
    InMemoryGrantStore, PermissionGrant, PermissionGrantError, PermissionId, PermissionIdError,
    PermissionRequest, PermissionScope, UserRole,
};

pub use capabilities::{
    CapabilityAccessRegistry, CapabilityAccessRegistryError, CapabilityAccessRule,
    CapabilityAccessRuleError, CapabilityDispatchError, CapabilityDispatchOutcome,
    CapabilityDispatcher, CapabilityExecution, CapabilityId, CapabilityIdError,
    CapabilityInvocation, CapabilityProvider, CapabilityProviderError, CapabilityRegistry,
    CapabilityRegistryError,
};

pub use contributions::{
    CommandAction, CommandContribution, CommandContributionError, CommandRegistry,
    CommandRegistryError, Contribution, ContributionError, ContributionId, ContributionIdError,
    ContributionKind, ContributionKindError, ContributionRegistry, ContributionRegistryError,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_explicit_file_permission_grant() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let file = ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("note.md").unwrap(),
        );

        let subject = app.clone().into();

        let grant = PermissionGrant::new(
            subject,
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file.clone()],
            app.into(),
        )
        .unwrap();

        assert_eq!(grant.permission().as_str(), "rumahl.files.read");

        assert_eq!(grant.resources(), &[file]);
    }
}
