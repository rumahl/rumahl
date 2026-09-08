use rumahl_core::{
    AppId,
    AppIdentity,
    AuthorizationDecision,
    AuthorizationDenyReason,
    AuthorizationEngine,
    CapabilityAccessRegistry,
    CapabilityAccessRule,
    CapabilityId,
    CapabilityInvocation,
    CapabilityProvider,
    CapabilityRegistry,
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

fn app(
    id: &str,
) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(
            "com.rumahl"
        )
        .unwrap(),
    )
}

fn file(
    key: &str,
) -> ResourceRef {
    ResourceRef::new(
        ResourceNamespace::parse(
            "rumahl.files"
        )
        .unwrap(),
        ResourceKind::parse(
            "file"
        )
        .unwrap(),
        ResourceKey::parse(key).unwrap(),
    )
}

#[test]
fn capability_invocation_is_authorized_through_permission_engine() {
    let consumer =
        app("com.rumahl.notes");

    let files =
        app("com.rumahl.files");

    let user =
        UserIdentity::new(
            UserId::new()
        );

    let user_id =
        *user.id();

    let allowed_file =
        file("document-1");

    let denied_file =
        file("document-2");

    /*
     * Register Files as provider for
     * rumahl.files.preview.
     */

    let capability =
        CapabilityId::parse(
            "rumahl.files.preview"
        )
        .unwrap();

    let files_identity =
        files.clone().into();

    let provider =
        CapabilityProvider::new(
            files.into(),
            capability.clone(),
        )
        .unwrap();

    let mut registry =
        CapabilityRegistry::new();

    registry
        .register(provider)
        .unwrap();

    let resolved_provider =
        registry
            .provider(
                &capability,
                &files_identity,
            )
            .unwrap()
            .clone();

    /*
     * User grants Notes read access
     * only to document-1.
     */

    let mut issuer_policy =
        GrantIssuerPolicy::new();

    issuer_policy.set_user_role(
        user_id,
        UserRole::User,
    );

    let authority =
        GrantAuthority::new();

    let grant = authority
        .issue(
            &issuer_policy,
            user.into(),
            consumer.clone().into(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
            PermissionScope::Explicit,
            vec![allowed_file.clone()],
        )
        .unwrap();

    let mut store =
        InMemoryGrantStore::new();

    store.insert(grant);

    /*
     * Notes invokes Files preview
     * in the user's context.
     */

    let invocation =
        CapabilityInvocation::new(
            OperationContext::for_app_as_user(
                consumer,
                user_id,
                SessionId::new(),
            ),
            resolved_provider,
        );

    /*
    * Trusted platform access rules.
    *
    * rumahl.files.preview
    * requires
    * rumahl.files.read
    */

    let access_rule =
        CapabilityAccessRule::new(
            capability.clone(),
            PermissionId::parse(
                "rumahl.files.read"
            )
            .unwrap(),
        );

    let mut access_registry =
        CapabilityAccessRegistry::new();

    access_registry
        .register(access_rule)
        .unwrap();

    let resolved_rule =
        access_registry
            .rule_for(&capability)
            .unwrap();

    let engine = AuthorizationEngine::new();

    /*
     * document-1 was granted.
     */

    let allowed_request =
        resolved_rule
            .authorization_request(
                &invocation,
                Some(allowed_file),
            )
            .unwrap();

    assert_eq!(
        engine.authorize(
            &allowed_request,
            store.grants(),
        ),
        AuthorizationDecision::Allow
    );

    /*
     * document-2 was not granted.
     */

    let denied_request =
        resolved_rule
            .authorization_request(
                &invocation,
                Some(denied_file),
            )
            .unwrap();

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