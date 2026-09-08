use crate::identity::Identity;
use crate::resources::ResourceRef;

use super::{
    GrantId,
    PermissionId,
    PermissionScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionGrant {
    id: GrantId,
    subject: Identity,
    permission: PermissionId,
    scope: PermissionScope,
    resources: Vec<ResourceRef>,
    granted_by: Identity,
}

impl PermissionGrant {
    pub fn new(
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
    ) -> Self {
        Self {
            id: GrantId::new(),
            subject,
            permission,
            scope,
            resources,
            granted_by,
        }
    }

    pub fn id(&self) -> &GrantId {
        &self.id
    }

    pub fn subject(&self) -> &Identity {
        &self.subject
    }

    pub fn permission(&self) -> &PermissionId {
        &self.permission
    }

    pub fn scope(&self) -> PermissionScope {
        self.scope
    }

    pub fn resources(&self) -> &[ResourceRef] {
        &self.resources
    }

    pub fn granted_by(&self) -> &Identity {
        &self.granted_by
    }
}