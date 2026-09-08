pub mod context;
pub mod identity;
pub mod resources;
pub mod permissions;

pub use context::{
    CorrelationId,
    CorrelationIdError,
    OperationContext,
};

pub use identity::{
    AppId,
    AppIdError,
    AppIdentity,
    Identity,
    InstallationId,
    InstallationIdError,
    PublisherId,
    PublisherIdError,
    ServiceId,
    ServiceIdError,
    ServiceIdentity,
    SessionId,
    SessionIdError,
    UserId,
    UserIdError,
    UserIdentity,
};

pub use resources::{
    ResourceKey,
    ResourceKeyError,
    ResourceKind,
    ResourceKindError,
    ResourceNamespace,
    ResourceNamespaceError,
    ResourceRef,
};

pub use permissions::{
    AuthorizationDecision,
    AuthorizationRequest,
    GrantId,
    PermissionGrant,
    PermissionId,
    PermissionIdError,
    PermissionRequest,
    PermissionScope,
};

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId,
        AppIdentity,
        InstallationId,
        OperationContext,
        PublisherId,
        ResourceKey,
        ResourceKind,
        ResourceNamespace,
        ResourceRef,
        SessionId,
        UserId,
    };

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
            PermissionId::parse(
                "rumahl.files.read",
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![file.clone()],
            app.into(),
        );

        assert_eq!(
            grant.permission().as_str(),
            "rumahl.files.read"
        );

        assert_eq!(
            grant.resources(),
            &[file]
        );
    }
}