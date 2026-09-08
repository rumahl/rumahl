use super::{
    AuthorizationDecision,
    AuthorizationDenyReason,
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
        let actor = request.context().actor();

        let mut has_actor_grant = false;
        let mut has_permission_grant = false;

        let mut resource_required = false;
        let mut resource_outside_scope = false;
        let mut unsupported_scope = false;

        for grant in &self.grants {
            if grant.subject() != actor {
                continue;
            }

            has_actor_grant = true;

            if grant.permission() != request.permission() {
                continue;
            }

            has_permission_grant = true;

            match grant.scope() {
                PermissionScope::System => {
                    return AuthorizationDecision::Allow;
                }

                PermissionScope::Explicit => {
                    let Some(resource) = request.resource() else {
                        resource_required = true;
                        continue;
                    };

                    if grant.resources().contains(resource) {
                        return AuthorizationDecision::Allow;
                    }

                    resource_outside_scope = true;
                }

                PermissionScope::AppPrivate
                | PermissionScope::UserOwn
                | PermissionScope::UserSelected
                | PermissionScope::FamilyShared => {
                    unsupported_scope = true;
                }
            }
        }

        if !has_actor_grant {
            return AuthorizationDecision::Deny(
                AuthorizationDenyReason::NoGrantForActor,
            );
        }

        if !has_permission_grant {
            return AuthorizationDecision::Deny(
                AuthorizationDenyReason::PermissionNotGranted,
            );
        }

        if resource_required {
            return AuthorizationDecision::Deny(
                AuthorizationDenyReason::ResourceRequired,
            );
        }

        if resource_outside_scope {
            return AuthorizationDecision::Deny(
                AuthorizationDenyReason::ResourceOutsideScope,
            );
        }

        if unsupported_scope {
            return AuthorizationDecision::Deny(
                AuthorizationDenyReason::UnsupportedScope,
            );
        }

        AuthorizationDecision::Deny(
            AuthorizationDenyReason::PermissionNotGranted,
        )
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
        let target = file("document-1");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target.clone()],
            app.clone().into(),
        )
        .unwrap();

        let context = OperationContext::for_app_as_user(
            app,
            UserId::new(),
            SessionId::new(),
        );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(target),
        );

        let mut engine = AuthorizationEngine::new();
        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Allow
        );
    }

    #[test]
    fn denies_resource_not_in_explicit_grant() {
        let app = notes_app();

        let allowed_file = file("document-1");
        let denied_file = file("private-document");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![allowed_file],
            app.clone().into(),
        )
        .unwrap();

        let context = OperationContext::for_app_as_user(
            app,
            UserId::new(),
            SessionId::new(),
        );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(denied_file),
        );

        let mut engine = AuthorizationEngine::new();
        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny(
                AuthorizationDenyReason::ResourceOutsideScope
            )
        );
    }

    #[test]
    fn explicit_scope_requires_resource() {
        let app = notes_app();
        let target = file("document-1");

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target],
            app.clone().into(),
        )
        .unwrap();

        let context = OperationContext::for_app_as_user(
            app,
            UserId::new(),
            SessionId::new(),
        );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            None,
        );

        let mut engine = AuthorizationEngine::new();
        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny(
                AuthorizationDenyReason::ResourceRequired
            )
        );
    }

    #[test]
    fn denies_unsupported_scope() {
        let app = notes_app();

        let grant = PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::UserOwn,
            vec![],
            app.clone().into(),
        )
        .unwrap();

        let context = OperationContext::for_app_as_user(
            app,
            UserId::new(),
            SessionId::new(),
        );

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );

        let mut engine = AuthorizationEngine::new();
        engine.add_grant(grant);

        assert_eq!(
            engine.authorize(&request),
            AuthorizationDecision::Deny(
                AuthorizationDenyReason::UnsupportedScope
            )
        );
    }
}