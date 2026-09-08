use std::error::Error;
use std::fmt;

use crate::{
    AuthorizationRequest,
    PermissionId,
    ResourceRef,
};

use super::{
    CapabilityId,
    CapabilityInvocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAccessRule {
    capability: CapabilityId,
    permission: PermissionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAccessRuleError {
    CapabilityMismatch,
}

impl CapabilityAccessRule {
    pub fn new(
        capability: CapabilityId,
        permission: PermissionId,
    ) -> Self {
        Self {
            capability,
            permission,
        }
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn permission(&self) -> &PermissionId {
        &self.permission
    }

    pub fn authorization_request(
        &self,
        invocation: &CapabilityInvocation,
        resource: Option<ResourceRef>,
    ) -> Result<
        AuthorizationRequest,
        CapabilityAccessRuleError,
    > {
        if invocation.capability() != &self.capability {
            return Err(
                CapabilityAccessRuleError::CapabilityMismatch
            );
        }

        Ok(
            AuthorizationRequest::new(
                invocation.context().clone(),
                self.permission.clone(),
                resource,
            )
        )
    }
}

impl fmt::Display for CapabilityAccessRuleError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::CapabilityMismatch => {
                write!(
                    f,
                    "capability invocation does not match access rule"
                )
            }
        }
    }
}

impl Error for CapabilityAccessRuleError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId,
        AppIdentity,
        CapabilityProvider,
        InstallationId,
        OperationContext,
        PublisherId,
        ResourceKey,
        ResourceKind,
        ResourceNamespace,
    };

    fn app(
        id: &str,
    ) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse(
                "com.rumahl"
            )
            .unwrap(),
        )
    }

    fn file(
        key: &str,
    ) -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse(
                "rumahl.files"
            )
            .unwrap(),
            ResourceKind::parse(
                "file"
            )
            .unwrap(),
            ResourceKey::parse(key).unwrap(),
        )
    }

    #[test]
    fn creates_authorization_request_for_matching_capability() {
        let consumer =
            app("com.rumahl.notes");

        let provider =
            app("com.rumahl.files");

        let capability =
            CapabilityId::parse(
                "rumahl.files.preview"
            )
            .unwrap();

        let capability_provider =
            CapabilityProvider::new(
                provider.into(),
                capability.clone(),
            )
            .unwrap();

        let invocation =
            CapabilityInvocation::new(
                OperationContext::for_background_app(
                    consumer,
                ),
                capability_provider,
            );

        let rule =
            CapabilityAccessRule::new(
                capability,
                PermissionId::parse(
                    "rumahl.files.read"
                )
                .unwrap(),
            );

        let result =
            rule.authorization_request(
                &invocation,
                Some(file("document-1")),
            );

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_rule_for_different_capability() {
        let consumer =
            app("com.rumahl.notes");

        let provider =
            app("com.rumahl.files");

        let invocation_capability =
            CapabilityId::parse(
                "rumahl.files.preview"
            )
            .unwrap();

        let provider =
            CapabilityProvider::new(
                provider.into(),
                invocation_capability,
            )
            .unwrap();

        let invocation =
            CapabilityInvocation::new(
                OperationContext::for_background_app(
                    consumer,
                ),
                provider,
            );

        let wrong_rule =
            CapabilityAccessRule::new(
                CapabilityId::parse(
                    "rumahl.search.query"
                )
                .unwrap(),
                PermissionId::parse(
                    "rumahl.files.read"
                )
                .unwrap(),
            );

        assert_eq!(
            wrong_rule
                .authorization_request(
                    &invocation,
                    None,
                )
                .unwrap_err(),
            CapabilityAccessRuleError::CapabilityMismatch
        );
    }
}