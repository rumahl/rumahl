use rumahl_core::{
    AppId, AppIdentity, AuthorizationDecision, AuthorizationDenyReason, AuthorizationEngine,
    CapabilityAccessRegistry, CapabilityAccessRule, CapabilityDispatcher, CapabilityId,
    CapabilityInvocation, CapabilityProvider, CapabilityRegistry, GrantAuthority,
    GrantIssuerPolicy, InMemoryGrantStore, InstallationId, OperationContext, PermissionId,
    PermissionScope, PublisherId, ResourceKey, ResourceKind, ResourceNamespace, ResourceRef,
    SessionId, UserId, UserIdentity, UserRole,
};

fn app(id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(id).unwrap(),
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
fn dispatch_authorizes_only_granted_resource() {
    let notes = app("com.rumahl.notes");
    let files = app("com.rumahl.files");

    let user = UserIdentity::new(UserId::new());
    let user_id = *user.id();

    let allowed_file = file("document-1");
    let denied_file = file("document-2");

    /*
     * Files provides rumahl.files.preview.
     */

    let capability = CapabilityId::parse("rumahl.files.preview").unwrap();

    let files_identity = files.clone().into();

    let provider = CapabilityProvider::new(files.into(), capability.clone()).unwrap();

    let mut capability_registry = CapabilityRegistry::new();

    capability_registry.register(provider).unwrap();

    let resolved_provider = capability_registry
        .provider(&capability, &files_identity)
        .unwrap()
        .clone();

    /*
     * Trusted platform rule:
     *
     * rumahl.files.preview
     * requires
     * rumahl.files.read
     */

    let access_rule = CapabilityAccessRule::new(
        capability.clone(),
        PermissionId::parse("rumahl.files.read").unwrap(),
    );

    let mut access_registry = CapabilityAccessRegistry::new();

    access_registry.register(access_rule).unwrap();

    /*
     * The user is allowed to approve an explicit grant.
     */

    let mut issuer_policy = GrantIssuerPolicy::new();

    issuer_policy.set_user_role(user_id, UserRole::User);

    /*
     * User grants Notes read access only
     * to document-1.
     */

    let authority = GrantAuthority::new();

    let grant = authority
        .issue(
            &issuer_policy,
            user.into(),
            notes.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![allowed_file.clone()],
        )
        .unwrap();

    let mut grant_store = InMemoryGrantStore::new();

    grant_store.insert(grant);

    /*
     * Notes invokes the Files preview capability
     * in the user's context.
     */

    let invocation = CapabilityInvocation::new(
        OperationContext::for_app_as_user(notes, user_id, SessionId::new()),
        resolved_provider,
    );

    let authorization_engine = AuthorizationEngine::new();

    let dispatcher = CapabilityDispatcher::new();

    /*
     * document-1 was explicitly granted.
     */

    let allowed = dispatcher
        .authorize(
            &invocation,
            Some(allowed_file),
            &capability_registry,
            &access_registry,
            &authorization_engine,
            grant_store.grants(),
        )
        .unwrap();

    assert_eq!(allowed, AuthorizationDecision::Allow);

    /*
     * document-2 was not granted.
     */

    let denied = dispatcher
        .authorize(
            &invocation,
            Some(denied_file),
            &capability_registry,
            &access_registry,
            &authorization_engine,
            grant_store.grants(),
        )
        .unwrap();

    assert_eq!(
        denied,
        AuthorizationDecision::Deny(AuthorizationDenyReason::ResourceOutsideScope)
    );
}
