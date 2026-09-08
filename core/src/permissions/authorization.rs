use crate::context::OperationContext;
use crate::resources::ResourceRef;

use super::PermissionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest {
    context: OperationContext,
    permission: PermissionId,
    resource: Option<ResourceRef>,
}

impl AuthorizationRequest {
    pub fn new(
        context: OperationContext,
        permission: PermissionId,
        resource: Option<ResourceRef>,
    ) -> Self {
        Self {
            context,
            permission,
            resource,
        }
    }

    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn permission(&self) -> &PermissionId {
        &self.permission
    }

    pub fn resource(&self) -> Option<&ResourceRef> {
        self.resource.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDenyReason {
    NoGrantForActor,
    PermissionNotGranted,
    ResourceRequired,
    ResourceOutsideScope,
    UnsupportedScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision {
    Allow,
    Deny(AuthorizationDenyReason),
    RequireUserApproval,
    RequireReauthentication,
}