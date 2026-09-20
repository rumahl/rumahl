use crate::{
    GrantId, Identity, PermissionGrant, PermissionGrantError, PermissionId, PermissionScope,
    ResourceRef,
};

#[derive(Debug, Clone)]
pub struct PermissionGrantSnapshot {
    grant: PermissionGrant,
}

impl PermissionGrantSnapshot {
    pub fn new(
        id: GrantId,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
    ) -> Result<Self, PermissionGrantError> {
        let grant =
            PermissionGrant::restore(id, subject, permission, scope, resources, granted_by)?;

        Ok(Self { grant })
    }

    pub fn capture(grant: &PermissionGrant) -> Self {
        Self {
            grant: grant.clone(),
        }
    }

    pub fn id(&self) -> &GrantId {
        self.grant.id()
    }

    pub fn subject(&self) -> &Identity {
        self.grant.subject()
    }

    pub fn permission(&self) -> &PermissionId {
        self.grant.permission()
    }

    pub fn scope(&self) -> PermissionScope {
        self.grant.scope()
    }

    pub fn resources(&self) -> &[ResourceRef] {
        self.grant.resources()
    }

    pub fn granted_by(&self) -> &Identity {
        self.grant.granted_by()
    }

    pub(crate) fn grant(&self) -> &PermissionGrant {
        &self.grant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, InstallationId, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace, UserId, UserIdentity,
    };

    fn app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn resource() -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("document-1").unwrap(),
        )
    }

    #[test]
    fn rebuilds_validated_grant_with_existing_id() {
        let id = GrantId::new();

        let snapshot = PermissionGrantSnapshot::new(
            id,
            app().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource()],
            UserIdentity::new(UserId::new()).into(),
        )
        .unwrap();

        assert_eq!(snapshot.id(), &id);
        assert_eq!(snapshot.resources().len(), 1);
    }

    #[test]
    fn rejects_invalid_persisted_grant() {
        assert_eq!(
            PermissionGrantSnapshot::new(
                GrantId::new(),
                app().into(),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                Vec::new(),
                UserIdentity::new(UserId::new()).into(),
            )
            .unwrap_err(),
            PermissionGrantError::ExplicitScopeRequiresResources
        );
    }
}
