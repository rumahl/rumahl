use std::error::Error;
use std::fmt;

use crate::identity::Identity;
use crate::resources::ResourceRef;

use super::{GrantId, GrantValidity, PermissionId, PermissionScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionGrantError {
    ExplicitScopeRequiresResources,
    SystemScopeCannotContainResources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionGrant {
    id: GrantId,
    subject: Identity,
    permission: PermissionId,
    scope: PermissionScope,
    resources: Vec<ResourceRef>,
    granted_by: Identity,
    validity: GrantValidity,
}

impl PermissionGrant {
    pub(crate) fn new(
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
    ) -> Result<Self, PermissionGrantError> {
        Self::create(
            GrantId::new(),
            subject,
            permission,
            scope,
            resources,
            granted_by,
            GrantValidity::durable(),
        )
    }

    pub(crate) fn new_with_validity(
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
        validity: GrantValidity,
    ) -> Result<Self, PermissionGrantError> {
        Self::create(
            GrantId::new(),
            subject,
            permission,
            scope,
            resources,
            granted_by,
            validity,
        )
    }

    pub(crate) fn restore(
        id: GrantId,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
    ) -> Result<Self, PermissionGrantError> {
        Self::restore_with_validity(
            id,
            subject,
            permission,
            scope,
            resources,
            granted_by,
            GrantValidity::durable(),
        )
    }

    pub(crate) fn restore_with_validity(
        id: GrantId,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
        validity: GrantValidity,
    ) -> Result<Self, PermissionGrantError> {
        Self::create(
            id, subject, permission, scope, resources, granted_by, validity,
        )
    }

    fn create(
        id: GrantId,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
        granted_by: Identity,
        validity: GrantValidity,
    ) -> Result<Self, PermissionGrantError> {
        if scope == PermissionScope::Explicit && resources.is_empty() {
            return Err(PermissionGrantError::ExplicitScopeRequiresResources);
        }

        if scope == PermissionScope::System && !resources.is_empty() {
            return Err(PermissionGrantError::SystemScopeCannotContainResources);
        }

        Ok(Self {
            id,
            subject,
            permission,
            scope,
            resources,
            granted_by,
            validity,
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

    /// How long the grant stays usable; see [`GrantValidity`].
    pub fn validity(&self) -> &GrantValidity {
        &self.validity
    }

    /// The same grant over a different resource set, keeping id, subject,
    /// permission, scope, issuer and lifetime. Used by storage migrations that
    /// translate resource keys; the consent itself is unchanged.
    pub fn with_resources(
        &self,
        resources: Vec<ResourceRef>,
    ) -> Result<Self, PermissionGrantError> {
        Self::restore_with_validity(
            self.id,
            self.subject.clone(),
            self.permission.clone(),
            self.scope,
            resources,
            self.granted_by.clone(),
            self.validity.clone(),
        )
    }
}

impl fmt::Display for PermissionGrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitScopeRequiresResources => {
                write!(f, "explicit scope requires at least one resource")
            }

            Self::SystemScopeCannotContainResources => {
                write!(f, "system scope cannot contain explicit resources")
            }
        }
    }
}

impl Error for PermissionGrantError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, InstallationId, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace, ResourceRef, SessionId, UnixTimestamp, UserId, UserIdentity,
    };

    fn test_app_identity() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn test_user_identity() -> UserIdentity {
        UserIdentity::new(UserId::new())
    }

    fn test_file_resource() -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("document-1").unwrap(),
        )
    }

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

    #[test]
    fn rejects_system_scope_with_resources() {
        let result = PermissionGrant::new(
            test_app_identity().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::System,
            vec![test_file_resource()],
            test_user_identity().into(),
        );

        assert_eq!(
            result.unwrap_err(),
            PermissionGrantError::SystemScopeCannotContainResources
        );
    }

    #[test]
    fn accepts_system_scope_without_resources() {
        let result = PermissionGrant::new(
            test_app_identity().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::System,
            Vec::new(),
            test_user_identity().into(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn restores_existing_grant_id() {
        let app = notes_app();
        let id = GrantId::new();

        let grant = PermissionGrant::restore(
            id,
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![test_file_resource()],
            app.into(),
        )
        .unwrap();

        assert_eq!(grant.id(), &id);
        assert_eq!(grant.validity(), &GrantValidity::durable());
    }

    #[test]
    fn grants_carry_their_validity() {
        let app = notes_app();
        let session = SessionId::new();

        let session_grant = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![test_file_resource()],
            app.clone().into(),
            GrantValidity::session(session),
        )
        .unwrap();

        assert_eq!(session_grant.validity(), &GrantValidity::session(session));

        let expires_at = UnixTimestamp::from_seconds(1_700_000_000);
        let expiring = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.write").unwrap(),
            PermissionScope::Explicit,
            vec![test_file_resource()],
            app.into(),
            GrantValidity::durable_until(expires_at),
        )
        .unwrap();

        assert_eq!(
            expiring.validity(),
            &GrantValidity::durable_until(expires_at)
        );
    }

    #[test]
    fn remapping_resources_keeps_id_and_consent() {
        let app = notes_app();
        let resource = test_file_resource();
        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource.clone()],
            app.clone().into(),
        )
        .unwrap();
        let replacement = ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("document-2").unwrap(),
        );

        let remapped = grant.with_resources(vec![replacement.clone()]).unwrap();

        assert_eq!(remapped.id(), grant.id());
        assert_eq!(remapped.permission(), grant.permission());
        assert_eq!(remapped.resources(), &[replacement]);
        // An explicit grant still needs at least one resource.
        assert_eq!(
            grant.with_resources(Vec::new()).unwrap_err(),
            PermissionGrantError::ExplicitScopeRequiresResources
        );
    }

    #[test]
    fn explicit_scope_still_requires_resources_with_validity() {
        let app = notes_app();

        let result = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![],
            app.into(),
            GrantValidity::session(SessionId::new()),
        );

        assert_eq!(
            result.unwrap_err(),
            PermissionGrantError::ExplicitScopeRequiresResources
        );
    }
}
