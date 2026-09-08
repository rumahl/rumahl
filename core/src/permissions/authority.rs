use std::error::Error;
use std::fmt;

use crate::identity::Identity;
use crate::resources::ResourceRef;

use super::{
    GrantIssuerPolicy,
    GrantIssuerPolicyError,
    PermissionGrant,
    PermissionGrantError,
    PermissionId,
    PermissionScope,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct GrantAuthority;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantAuthorityError {
    IssuerNotAuthorized(GrantIssuerPolicyError),
    InvalidGrant(PermissionGrantError),
}

impl GrantAuthority {
    pub fn new() -> Self {
        Self
    }

    pub fn issue(
        &self,
        policy: &GrantIssuerPolicy,
        issuer: Identity,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
    ) -> Result<PermissionGrant, GrantAuthorityError> {
        policy
            .authorize_issuer(&issuer, scope)
            .map_err(
                GrantAuthorityError::IssuerNotAuthorized
            )?;

        PermissionGrant::new(
            subject,
            permission,
            scope,
            resources,
            issuer,
        )
        .map_err(GrantAuthorityError::InvalidGrant)
    }
}

impl fmt::Display for GrantAuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IssuerNotAuthorized(error) => {
                write!(
                    f,
                    "grant issuer is not authorized: {error}"
                )
            }

            Self::InvalidGrant(error) => {
                write!(
                    f,
                    "cannot issue invalid permission grant: {error}"
                )
            }
        }
    }
}

impl Error for GrantAuthorityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IssuerNotAuthorized(error) => Some(error),
            Self::InvalidGrant(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId,
        AppIdentity,
        InstallationId,
        PublisherId,
        ResourceKey,
        ResourceKind,
        ResourceNamespace,
        ServiceId,
        ServiceIdentity,
        UserRole,
        UserId,
        UserIdentity,
    };

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
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

    #[test]
    fn authorized_user_can_issue_grant() {
        let authority =
            GrantAuthority::new();

        let user =
            UserIdentity::new(UserId::new());

        let user_id =
            *user.id();

        let mut policy =
            GrantIssuerPolicy::new();

        policy.set_user_access(
            user_id,
            UserRole::User,
        );

        let result = authority.issue(
            &policy,
            user.into(),
            notes_app().into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn trusted_service_can_issue_grant() {
        let authority =
            GrantAuthority::new();

        let service_id =
            ServiceId::parse(
                "rumahl.permission-service"
            )
            .unwrap();

        let service =
            ServiceIdentity::new(
                service_id.clone()
            );

        let mut policy =
            GrantIssuerPolicy::new();

        policy.trust_service(service_id);

        let result = authority.issue(
            &policy,
            service.into(),
            notes_app().into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn app_cannot_issue_grant() {
        let authority =
            GrantAuthority::new();

        let policy =
            GrantIssuerPolicy::new();

        let issuer =
            notes_app();

        let subject = AppIdentity::new(
            AppId::parse(
                "com.example.reader"
            )
            .unwrap(),
            InstallationId::new(),
            PublisherId::parse(
                "com.example"
            )
            .unwrap(),
        );

        let result = authority.issue(
            &policy,
            issuer.into(),
            subject.into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
        );

        assert_eq!(
            result.unwrap_err(),
            GrantAuthorityError::IssuerNotAuthorized(
                GrantIssuerPolicyError::AppCannotIssueGrant
            )
        );
    }

    #[test]
    fn rejects_invalid_grant() {
        let authority =
            GrantAuthority::new();

        let user =
            UserIdentity::new(UserId::new());

        let user_id =
            *user.id();

        let mut policy =
            GrantIssuerPolicy::new();

        policy.set_user_access(
            user_id,
            UserRole::User,
        );

        let result = authority.issue(
            &policy,
            user.into(),
            notes_app().into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![],
        );

        assert_eq!(
            result.unwrap_err(),
            GrantAuthorityError::InvalidGrant(
                PermissionGrantError::ExplicitScopeRequiresResources
            )
        );
    }
}