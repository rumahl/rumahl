use super::{
    AuthorizationDecision, AuthorizationDenyReason, AuthorizationRequest, PermissionGrant,
    PermissionScope,
};
use crate::UnixTimestamp;

#[derive(Debug, Default, Clone, Copy)]
pub struct AuthorizationEngine;
impl AuthorizationEngine {
    pub fn new() -> Self {
        Self
    }

    /// Authorizes with the current clock. A clock that cannot be read denies
    /// the request: no permission is ever granted without a trustworthy time.
    pub fn authorize(
        &self,
        request: &AuthorizationRequest,
        grants: &[PermissionGrant],
    ) -> AuthorizationDecision {
        match UnixTimestamp::now() {
            Ok(now) => self.authorize_at(request, grants, now),
            Err(_) => AuthorizationDecision::Deny(AuthorizationDenyReason::PermissionNotGranted),
        }
    }

    /// Authorizes against an explicit clock, so expiry and session binding are
    /// testable. Unusable grants — expired, or session grants outside their
    /// session — are treated exactly like missing grants.
    pub fn authorize_at(
        &self,
        request: &AuthorizationRequest,
        grants: &[PermissionGrant],
        now: UnixTimestamp,
    ) -> AuthorizationDecision {
        let actor = request.context().actor();
        let session = request.context().session();

        let mut has_actor_grant = false;
        let mut has_permission_grant = false;

        let mut resource_required = false;
        let mut resource_outside_scope = false;
        let mut unsupported_scope = false;

        for grant in grants {
            if !grant.validity().is_usable(session, now) {
                continue;
            }

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
            return AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor);
        }

        if !has_permission_grant {
            return AuthorizationDecision::Deny(AuthorizationDenyReason::PermissionNotGranted);
        }

        if resource_required {
            return AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceRequired);
        }

        if resource_outside_scope {
            return AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceOutsideScope);
        }

        if unsupported_scope {
            return AuthorizationDecision::Deny(AuthorizationDenyReason::UnsupportedScope);
        }

        AuthorizationDecision::Deny(AuthorizationDenyReason::PermissionNotGranted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, GrantValidity, InstallationId, OperationContext, PermissionId,
        PublisherId, ResourceKey, ResourceKind, ResourceNamespace, ResourceRef, SessionId,
        UnixTimestamp, UserId,
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

        let context = OperationContext::for_app_as_user(app, UserId::new(), SessionId::new());

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(target),
        );

        let engine = AuthorizationEngine::new();

        assert_eq!(
            engine.authorize(&request, &[grant],),
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

        let context = OperationContext::for_app_as_user(app, UserId::new(), SessionId::new());

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(denied_file),
        );

        let engine = AuthorizationEngine::new();

        assert_eq!(
            engine.authorize(&request, &[grant]),
            AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceOutsideScope)
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

        let context = OperationContext::for_app_as_user(app, UserId::new(), SessionId::new());

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            None,
        );

        let engine = AuthorizationEngine::new();

        assert_eq!(
            engine.authorize(&request, &[grant]),
            AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceRequired)
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

        let context = OperationContext::for_app_as_user(app, UserId::new(), SessionId::new());

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );

        let engine = AuthorizationEngine::new();

        assert_eq!(
            engine.authorize(&request, &[grant]),
            AuthorizationDecision::Deny(AuthorizationDenyReason::UnsupportedScope)
        );
    }

    #[test]
    fn expired_grant_denies_like_a_missing_grant() {
        let app = notes_app();
        let target = file("document-1");
        let now = UnixTimestamp::from_seconds(2_000);

        let grant = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target],
            app.clone().into(),
            GrantValidity::durable_until(UnixTimestamp::from_seconds(1_999)),
        )
        .unwrap();

        let context = OperationContext::for_app_as_user(app, UserId::new(), SessionId::new());

        let request = AuthorizationRequest::new(
            context,
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );

        let engine = AuthorizationEngine::new();

        // The same grant is still usable one second earlier, then it is gone.
        assert_eq!(
            engine.authorize_at(
                &request,
                std::slice::from_ref(&grant),
                UnixTimestamp::from_seconds(1_998)
            ),
            AuthorizationDecision::Allow
        );
        assert_eq!(
            engine.authorize_at(&request, std::slice::from_ref(&grant), now),
            AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor)
        );
    }

    #[test]
    fn session_grant_authorizes_only_its_session() {
        let app = notes_app();
        let session = SessionId::new();
        let other_session = SessionId::new();
        let target = file("document-1");
        let now = UnixTimestamp::from_seconds(1_000);

        let grant = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![target],
            app.clone().into(),
            GrantValidity::session(session),
        )
        .unwrap();

        let engine = AuthorizationEngine::new();

        let granted = AuthorizationRequest::new(
            OperationContext::for_app_as_user(app.clone(), UserId::new(), session),
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );
        assert_eq!(
            engine.authorize_at(&granted, std::slice::from_ref(&grant), now),
            AuthorizationDecision::Allow
        );

        // Another session of the same user never inherits the grant.
        let other = AuthorizationRequest::new(
            OperationContext::for_app_as_user(app.clone(), UserId::new(), other_session),
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );
        assert_eq!(
            engine.authorize_at(&other, std::slice::from_ref(&grant), now),
            AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor)
        );

        // And an operation without any session (background, recovery) neither.
        let background = AuthorizationRequest::new(
            OperationContext::for_background_app(app),
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("document-1")),
        );
        assert_eq!(
            engine.authorize_at(&background, std::slice::from_ref(&grant), now),
            AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor)
        );
    }

    #[test]
    fn session_grant_never_authorizes_a_resource_outside_its_scope() {
        let app = notes_app();
        let session = SessionId::new();

        let grant = PermissionGrant::new_with_validity(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file("document-1")],
            app.clone().into(),
            GrantValidity::session(session),
        )
        .unwrap();

        let request = AuthorizationRequest::new(
            OperationContext::for_app_as_user(app, UserId::new(), session),
            PermissionId::parse("rumahl.files.read").unwrap(),
            Some(file("private-document")),
        );

        let engine = AuthorizationEngine::new();

        assert_eq!(
            engine.authorize_at(&request, &[grant], UnixTimestamp::from_seconds(1)),
            AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceOutsideScope)
        );
    }
}
