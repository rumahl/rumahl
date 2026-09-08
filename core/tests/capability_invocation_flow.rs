use rumahl_core::{
    AppId,
    AppIdentity,
    CapabilityId,
    CapabilityInvocation,
    CapabilityProvider,
    CapabilityRegistry,
    InstallationId,
    OperationContext,
    PublisherId,
};

fn app(
    app_id: &str,
    publisher_id: &str,
) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
    )
}

#[test]
fn registered_provider_can_be_resolved_for_invocation() {
    let consumer = app(
        "com.rumahl.notes",
        "com.rumahl",
    );

    let provider_app = app(
        "com.rumahl.files",
        "com.rumahl",
    );

    let provider_identity =
        provider_app.clone().into();

    let capability =
        CapabilityId::parse(
            "rumahl.files.preview"
        )
        .unwrap();

    let provider =
        CapabilityProvider::new(
            provider_app.into(),
            capability.clone(),
        )
        .unwrap();

    let mut registry =
        CapabilityRegistry::new();

    registry
        .register(provider)
        .unwrap();

    let resolved = registry
        .provider(
            &capability,
            &provider_identity,
        )
        .unwrap();

    let context =
        OperationContext::for_background_app(
            consumer,
        );

    let invocation =
        CapabilityInvocation::new(
            context,
            resolved.clone(),
        );

    assert_eq!(
        invocation.capability(),
        &capability
    );

    assert_eq!(
        invocation.provider(),
        &provider_identity
    );
}

#[test]
fn provider_cannot_be_resolved_for_unregistered_capability() {
    let provider_app = app(
        "com.rumahl.files",
        "com.rumahl",
    );

    let provider_identity =
        provider_app.clone().into();

    let registered_capability =
        CapabilityId::parse(
            "rumahl.files.preview"
        )
        .unwrap();

    let requested_capability =
        CapabilityId::parse(
            "rumahl.search.query"
        )
        .unwrap();

    let provider =
        CapabilityProvider::new(
            provider_app.into(),
            registered_capability,
        )
        .unwrap();

    let mut registry =
        CapabilityRegistry::new();

    registry
        .register(provider)
        .unwrap();

    assert!(
        registry
            .provider(
                &requested_capability,
                &provider_identity,
            )
            .is_none()
    );
}