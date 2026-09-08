use super::{
    AuthorizationDecision,
    AuthorizationRequest,
    PermissionGrant,
    PermissionScope,
};

#[derive(Debug, Default)]
pub struct AuthorizationEngine {
    grants: Vec<PermissionGrant>,
}

impl AuthorizationEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_grant(&mut self, grant: PermissionGrant) {
        self.grants.push(grant);
    }

    pub fn grants(&self) -> &[PermissionGrant] {
        &self.grants
    }

    pub fn authorize(
        &self,
        request: &AuthorizationRequest,
    ) -> AuthorizationDecision {
        for grant in &self.grants {
            if grant.subject() != request.context().actor() {
                continue;
            }

            if grant.permission() != request.permission() {
                continue;
            }

            if Self::grant_matches_request(grant, request) {
                return AuthorizationDecision::Allow;
            }
        }

        AuthorizationDecision::Deny
    }

    fn grant_matches_request(
        grant: &PermissionGrant,
        request: &AuthorizationRequest,
    ) -> bool {
        match grant.scope() {
            PermissionScope::Explicit => {
                let Some(resource) = request.resource() else {
                    return false;
                };

                grant.resources().contains(resource)
            }

            PermissionScope::System => true,

            PermissionScope::AppPrivate
            | PermissionScope::UserOwn
            | PermissionScope::UserSelected
            | PermissionScope::FamilyShared => false,
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
        OperationContext,
        PermissionId,
        PermissionScope,
        PublisherId,
        ResourceKey,
        ResourceKind,
        ResourceNamespace,
        ResourceRef,
        SessionId,
        UserId,
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
    fn allows_explicitly_granted_resource() {
        let app = notes_app();

        let allowed_file =
            file("document-1");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![allowed_file.clone()],
            app.clone().into(),
        );

        let context =
            OperationContext::for_app_as_user(
                app,
                UserId::new(),
                SessionId::new(),
            );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(allowed_file),
        );

        let mut engine =
            AuthorizationEngine::new();

        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Allow
        );
    }

        #[test]
    fn denies_resource_not_in_explicit_grant() {
        let app = notes_app();

        let allowed_file =
            file("document-1");

        let denied_file =
            file("private-document");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![allowed_file],
            app.clone().into(),
        );

        let context =
            OperationContext::for_app_as_user(
                app,
                UserId::new(),
                SessionId::new(),
            );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(denied_file),
        );

        let mut engine =
            AuthorizationEngine::new();

        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny
        );
    }

        #[test]
    fn denies_permission_not_granted() {
        let app = notes_app();

        let target =
            file("document-1");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target.clone()],
            app.clone().into(),
        );

        let context =
            OperationContext::for_app_as_user(
                app,
                UserId::new(),
                SessionId::new(),
            );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.write").unwrap(),
            Some(target),
        );

        let mut engine =
            AuthorizationEngine::new();

        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny
        );
    }

        #[test]
    fn denies_grant_owned_by_different_actor() {
        let notes = notes_app();

        let other_app = AppIdentity::new(
            AppId::parse("com.example.reader").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.example").unwrap(),
        );

        let target =
            file("document-1");

        let grant = PermissionGrant::new(
            notes.into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target.clone()],
            other_app.clone().into(),
        );

        let context =
            OperationContext::for_app_as_user(
                other_app,
                UserId::new(),
                SessionId::new(),
            );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(target),
        );

        let mut engine =
            AuthorizationEngine::new();

        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny
        );
    }
}