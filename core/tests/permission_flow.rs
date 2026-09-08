use rumahl_core::{
    AppId,
    AppIdentity,
    AuthorizationDecision,
    AuthorizationDenyReason,
    AuthorizationEngine,
    AuthorizationRequest,
    GrantAuthority,
    GrantIssuerPolicy,
    InMemoryGrantStore,
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
    UserIdentity,
    UserRole,
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
fn approved_explicit_permission_allows_only_selected_resource() {
    let app = notes_app();

    let user =
        UserIdentity::new(UserId::new());

    let user_id =
        *user.id();

    let allowed_resource =
        file("document-1");

    let denied_resource =
        file("document-2");

    let mut policy =
        GrantIssuerPolicy::new();

    policy.set_user_role(
        user_id,
        UserRole::User,
    );

    let authority =
        GrantAuthority::new();

    let grant = authority
        .issue(
            &policy,
            user.into(),
            app.clone().into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![allowed_resource.clone()],
        )
        .unwrap();

    let mut store =
        InMemoryGrantStore::new();

    store.insert(grant);

    assert_eq!(store.len(), 1);

    let engine =
        AuthorizationEngine::new();

    let allowed_request =
        AuthorizationRequest::new(
            OperationContext::for_app_as_user(
                app.clone(),
                user_id,
                SessionId::new(),
            ),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            Some(allowed_resource),
        );

    assert_eq!(
        engine.authorize(
            &allowed_request,
            store.grants(),
        ),
        AuthorizationDecision::Allow
    );

    let denied_request =
        AuthorizationRequest::new(
            OperationContext::for_app_as_user(
                app,
                user_id,
                SessionId::new(),
            ),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            Some(denied_resource),
        );

    assert_eq!(
        engine.authorize(
            &denied_request,
            store.grants(),
        ),
        AuthorizationDecision::Deny(
            AuthorizationDenyReason::ResourceOutsideScope
        )
    );
}