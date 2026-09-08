use rumahl_core::{
    AppId, AppIdentity, AuthorizationDecision, AuthorizationEngine, CapabilityAccessRegistry,
    CapabilityAccessRule, CapabilityDispatchOutcome, CapabilityDispatcher, CapabilityId,
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
fn authorized_dispatch_produces_execution_handoff() {
    let notes = app("com.rumahl.notes");

    let files = app("com.rumahl.files");

    let user = UserIdentity::new(UserId::new());

    let user_id = *user.id();

    let resource = file("document-1");

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
     * Platform rule:
     *
     * rumahl.files.preview
     * requires
     * rumahl.files.read
     */

    let mut access_registry = CapabilityAccessRegistry::new();

    access_registry
        .register(CapabilityAccessRule::new(
            capability.clone(),
            PermissionId::parse("rumahl.files.read").unwrap(),
        ))
        .unwrap();

    /*
     * User may approve explicit grants.
     */

    let mut issuer_policy = GrantIssuerPolicy::new();

    issuer_policy.set_user_role(user_id, UserRole::User);

    /*
     * User grants Notes access to document-1.
     */

    let authority = GrantAuthority::new();

    let grant = authority
        .issue(
            &issuer_policy,
            user.into(),
            notes.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![resource.clone()],
        )
        .unwrap();

    let mut grant_store = InMemoryGrantStore::new();

    grant_store.insert(grant);

    /*
     * Notes invokes the capability
     * in the user's context.
     */

    let invocation = CapabilityInvocation::new(
        OperationContext::for_app_as_user(notes, user_id, SessionId::new()),
        resolved_provider,
    );

    let engine = AuthorizationEngine::new();

    let dispatcher = CapabilityDispatcher::new();

    let outcome = dispatcher
        .prepare_execution(
            &invocation,
            Some(resource.clone()),
            &capability_registry,
            &access_registry,
            &engine,
            grant_store.grants(),
        )
        .unwrap();

    let CapabilityDispatchOutcome::Ready(execution) = outcome else {
        panic!("expected ready capability execution");
    };

    assert_eq!(execution.capability(), &capability);

    assert_eq!(execution.resource(), Some(&resource));

    assert_eq!(execution.provider(), invocation.capability_provider());
}

#[test]
fn denied_dispatch_does_not_produce_execution_handoff() {
    let notes = app("com.rumahl.notes");

    let files = app("com.rumahl.files");

    let resource = file("document-1");

    let capability = CapabilityId::parse("rumahl.files.preview").unwrap();

    let provider = CapabilityProvider::new(files.into(), capability.clone()).unwrap();

    let mut capability_registry = CapabilityRegistry::new();

    capability_registry.register(provider.clone()).unwrap();

    let mut access_registry = CapabilityAccessRegistry::new();

    access_registry
        .register(CapabilityAccessRule::new(
            capability,
            PermissionId::parse("rumahl.files.read").unwrap(),
        ))
        .unwrap();

    let invocation =
        CapabilityInvocation::new(OperationContext::for_background_app(notes), provider);

    let engine = AuthorizationEngine::new();

    let dispatcher = CapabilityDispatcher::new();

    let outcome = dispatcher
        .prepare_execution(
            &invocation,
            Some(resource),
            &capability_registry,
            &access_registry,
            &engine,
            &[],
        )
        .unwrap();

    assert!(matches!(
        outcome,
        CapabilityDispatchOutcome::NotAuthorized(AuthorizationDecision::Deny(_))
    ));
}
