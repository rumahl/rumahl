use std::error::Error;
use std::fmt;

use crate::identity::Identity;
use crate::resources::ResourceRef;

use super::{
    PermissionGrant,
    PermissionGrantError,
    PermissionId,
    PermissionScope,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct GrantAuthority;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantAuthorityError {
    AppCannotIssueGrant,
    InvalidGrant(PermissionGrantError),
}

impl GrantAuthority {
    pub fn new() -> Self {
        Self
    }

    pub fn issue(
        &self,
        issuer: Identity,
        subject: Identity,
        permission: PermissionId,
        scope: PermissionScope,
        resources: Vec<ResourceRef>,
    ) -> Result<PermissionGrant, GrantAuthorityError> {
        if issuer.is_app() {
            return Err(
                GrantAuthorityError::AppCannotIssueGrant
            );
        }

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
            Self::AppCannotIssueGrant => {
                write!(
                    f,
                    "apps are not allowed to issue permission grants"
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
            Self::AppCannotIssueGrant => None,
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
    fn user_can_issue_grant() {
        let authority = GrantAuthority::new();

        let user =
            UserIdentity::new(UserId::new());

        let app =
            notes_app();

        let resource =
            file("document-1");

        let result = authority.issue(
            user.into(),
            app.into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn service_can_issue_grant() {
        let authority = GrantAuthority::new();

        let service = ServiceIdentity::new(
            ServiceId::parse(
                "rumahl.permission-service"
            )
            .unwrap(),
        );

        let app =
            notes_app();

        let result = authority.issue(
            service.into(),
            app.into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn app_cannot_issue_grant() {
        let authority =
            GrantAuthority::new();

        let issuer =
            notes_app();

        let subject = AppIdentity::new(
            AppId::parse("com.example.reader").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.example").unwrap(),
        );

        let result = authority.issue(
            issuer.into(),
            subject.into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
        );

        assert_eq!(
            result.unwrap_err(),
            GrantAuthorityError::AppCannotIssueGrant
        );
    }

        #[test]
    fn rejects_invalid_grant() {
        let authority =
            GrantAuthority::new();

        let user =
            UserIdentity::new(UserId::new());

        let app =
            notes_app();

        let result = authority.issue(
            user.into(),
            app.into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
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