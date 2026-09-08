use std::error::Error;
use std::fmt;

use crate::identity::Identity;
use crate::resources::ResourceRef;

use super::{GrantId, PermissionId, PermissionScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionGrantError {
    ExplicitScopeRequiresResources,
}

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
    pub(crate) fn new(
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
    ) -> Result<Self, PermissionGrantError> {
        if scope == PermissionScope::Explicit && resources.is_empty() {
            return Err(PermissionGrantError::ExplicitScopeRequiresResources);
        }

        Ok(Self {
            id: GrantId::new(),
            subject,
            permission,
            scope,
            resources,
            granted_by,
        })
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

impl fmt::Display for PermissionGrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitScopeRequiresResources => {
                write!(
                    f,
                    "explicit permission scope requires at least one resource"
                )
            }
        }
    }
}

impl Error for PermissionGrantError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, InstallationId, PermissionId, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace, ResourceRef,
    };

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn rejects_explicit_grant_without_resources() {
        let app = notes_app();

        let result = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![],
            app.into(),
        );

        assert_eq!(
            result.unwrap_err(),
            PermissionGrantError::ExplicitScopeRequiresResources
        );
    }

    #[test]
    fn accepts_explicit_grant_with_resource() {
        let app = notes_app();

        let resource = ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("document-1").unwrap(),
        );

        let result = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource],
            app.into(),
        );

        assert!(result.is_ok());
    }
}
