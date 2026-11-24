use rumahl_core::{
    AppId, AppIdentity, AuthorizationDecision, AuthorizationDenyReason, AuthorizationEngine,
    AuthorizationRequest, GrantAuthority, GrantIssuerPolicy, GrantValidity, InMemoryGrantStore,
    InstallationId, OperationContext, PermissionId, PermissionScope, PublisherId, ResourceKey,
    ResourceKind, ResourceNamespace, ResourceRef, SessionId, UnixTimestamp, UserId, UserIdentity,
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

    let user = UserIdentity::new(UserId::new());

    let user_id = *user.id();

    let allowed_resource = file("document-1");

    let denied_resource = file("document-2");

    let mut policy = GrantIssuerPolicy::new();

    policy.set_user_role(user_id, UserRole::User);

    let authority = GrantAuthority::new();

    let grant = authority
        .issue(
            &policy,
            user.into(),
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![allowed_resource.clone()],
        )
        .unwrap();

    let mut store = InMemoryGrantStore::new();

    store.insert(grant);

    assert_eq!(store.len(), 1);

    let engine = AuthorizationEngine::new();

    let allowed_request = AuthorizationRequest::new(
        OperationContext::for_app_as_user(app.clone(), user_id, SessionId::new()),
        PermissionId::parse("rumahl.files.read").unwrap(),
        Some(allowed_resource),
    );

    assert_eq!(
        engine.authorize(&allowed_request, store.grants(),),
        AuthorizationDecision::Allow
    );

    let denied_request = AuthorizationRequest::new(
        OperationContext::for_app_as_user(app, user_id, SessionId::new()),
        PermissionId::parse("rumahl.files.read").unwrap(),
        Some(denied_resource),
    );

    assert_eq!(
        engine.authorize(&denied_request, store.grants(),),
        AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceOutsideScope)
    );
}

#[test]
fn a_temporary_session_grant_dies_with_its_session() {
    let app = notes_app();
    let user = UserIdentity::new(UserId::new());
    let user_id = *user.id();
    let consent_session = SessionId::new();
    let resource = file("picked-document");

    let mut policy = GrantIssuerPolicy::new();
    policy.set_user_role(user_id, UserRole::User);

    // The user consents for this browser session only.
    let grant = GrantAuthority::new()
        .issue_with_validity(
            &policy,
            user.into(),
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource.clone()],
            GrantValidity::session(consent_session),
        )
        .unwrap();

    let mut store = InMemoryGrantStore::new();
    store.insert(grant);

    let engine = AuthorizationEngine::new();

    let same_session = AuthorizationRequest::new(
        OperationContext::for_app_as_user(app.clone(), user_id, consent_session),
        PermissionId::parse("rumahl.files.read").unwrap(),
        Some(resource.clone()),
    );
    assert_eq!(
        engine.authorize(&same_session, store.grants()),
        AuthorizationDecision::Allow
    );

    // A different session of the same user — or any operation after the
    // session ended — never inherits the consent.
    let other_session = AuthorizationRequest::new(
        OperationContext::for_app_as_user(app, user_id, SessionId::new()),
        PermissionId::parse("rumahl.files.read").unwrap(),
        Some(resource),
    );
    assert_eq!(
        engine.authorize(&other_session, store.grants()),
        AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor)
    );
}

#[test]
fn an_expired_durable_grant_denies_like_a_missing_grant() {
    let app = notes_app();
    let user = UserIdentity::new(UserId::new());
    let user_id = *user.id();
    let resource = file("shared-report");

    let mut policy = GrantIssuerPolicy::new();
    policy.set_user_role(user_id, UserRole::User);

    let grant = GrantAuthority::new()
        .issue_with_validity(
            &policy,
            user.into(),
            app.clone().into(),
            PermissionId::parse("rumahl.files.write").unwrap(),
            PermissionScope::Explicit,
            vec![resource.clone()],
            GrantValidity::durable_until(UnixTimestamp::from_seconds(3_000)),
        )
        .unwrap();

    let mut store = InMemoryGrantStore::new();
    store.insert(grant);

    let engine = AuthorizationEngine::new();

    let request = AuthorizationRequest::new(
        OperationContext::for_app_as_user(app, user_id, SessionId::new()),
        PermissionId::parse("rumahl.files.write").unwrap(),
        Some(resource),
    );

    assert_eq!(
        engine.authorize_at(&request, store.grants(), UnixTimestamp::from_seconds(2_999)),
        AuthorizationDecision::Allow
    );
    assert_eq!(
        engine.authorize_at(&request, store.grants(), UnixTimestamp::from_seconds(3_000)),
        AuthorizationDecision::Deny(AuthorizationDenyReason::NoGrantForActor)
    );
}
