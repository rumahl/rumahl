use rumahl_core::{
    AppId, AppIdentity, EventBus, EventEnvelope, EventName, EventSubscription, Identity,
    InstallationId, OperationContext, PublisherId, ResourceKey, ResourceKind, ResourceNamespace,
    ResourceRef,
};

fn app(app_id: &str, publisher_id: &str) -> AppIdentity {
    AppIdentity::new(
        AppId::parse(app_id).unwrap(),
        InstallationId::new(),
        PublisherId::parse(publisher_id).unwrap(),
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
fn event_is_routed_to_all_matching_subscribers() {
    let files = app("com.rumahl.files", "com.rumahl");

    let notes = app("com.rumahl.notes", "com.rumahl");

    let search = app("com.rumahl.search", "com.rumahl");

    let backup = app("com.rumahl.backup", "com.rumahl");

    let files_identity: Identity = files.clone().into();

    let notes_identity: Identity = notes.clone().into();

    let search_identity: Identity = search.clone().into();

    let backup_identity: Identity = backup.clone().into();

    let files_changed = EventName::parse("rumahl.files.changed").unwrap();

    let apps_installed = EventName::parse("rumahl.apps.installed").unwrap();

    let mut bus = EventBus::new();

    /*
     * Notes and Search want file change events.
     */

    bus.subscribe(EventSubscription::new(notes.into(), files_changed.clone()).unwrap())
        .unwrap();

    bus.subscribe(EventSubscription::new(search.into(), files_changed.clone()).unwrap())
        .unwrap();

    /*
     * Backup subscribes to an unrelated event.
     */

    bus.subscribe(EventSubscription::new(backup.into(), apps_installed).unwrap())
        .unwrap();

    let resource = file("document-1");

    /*
     * Files produces one event.
     */

    let event = EventEnvelope::new(
        files_changed.clone(),
        OperationContext::for_background_app(files),
        Some(resource.clone()),
    );

    let event_id = event.id();

    /*
     * Core prepares transport-neutral deliveries.
     */

    let deliveries = bus.prepare_deliveries(&event);

    assert_eq!(deliveries.len(), 2);

    /*
     * Both deliveries represent the exact
     * same logical event instance.
     */

    assert!(
        deliveries
            .iter()
            .all(|delivery| { delivery.event_id() == event_id })
    );

    assert!(
        deliveries
            .iter()
            .all(|delivery| { delivery.event_name() == &files_changed })
    );

    assert!(
        deliveries
            .iter()
            .all(|delivery| { delivery.event().resource() == Some(&resource) })
    );

    /*
     * The original operation context survives
     * the routing handoff.
     */

    assert!(
        deliveries
            .iter()
            .all(|delivery| { delivery.event().context().actor() == &files_identity })
    );

    /*
     * Only matching subscribers receive
     * a delivery handoff.
     */

    assert!(
        deliveries
            .iter()
            .any(|delivery| { delivery.subscriber() == &notes_identity })
    );

    assert!(
        deliveries
            .iter()
            .any(|delivery| { delivery.subscriber() == &search_identity })
    );

    assert!(
        deliveries
            .iter()
            .all(|delivery| { delivery.subscriber() != &backup_identity })
    );
}

#[test]
fn unrelated_event_produces_no_delivery() {
    let files = app("com.rumahl.files", "com.rumahl");

    let notes = app("com.rumahl.notes", "com.rumahl");

    let mut bus = EventBus::new();

    bus.subscribe(
        EventSubscription::new(
            notes.into(),
            EventName::parse("rumahl.files.changed").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let event = EventEnvelope::new(
        EventName::parse("rumahl.apps.installed").unwrap(),
        OperationContext::for_background_app(files),
        None,
    );

    let deliveries = bus.prepare_deliveries(&event);

    assert!(deliveries.is_empty());
}
