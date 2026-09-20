use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rumahl_core::{
    AppId, AppLifecycle, AppManifest, AppVersion, AuthorizationEngine, CapabilityAccessRegistry,
    CapabilityAccessRule, CapabilityDispatchOutcome, CapabilityDispatcher, CapabilityExecution,
    CapabilityId, CapabilityInvocation, EventDelivery, EventEnvelope, EventName, GrantAuthority,
    GrantIssuerPolicy, Identity, InstalledApp, OperationContext, PackagePath, PermissionId,
    PermissionScope, PlatformState, PublisherId, ResourceKey, ResourceKind, ResourceNamespace,
    ResourceRef, RuntimeAdapter, RuntimeAdapterError, RuntimeAdapterRegistry, RuntimeDescriptor,
    RuntimeEndpointId, RuntimeEntrypoint, RuntimeEntrypointId, RuntimeKind, RuntimeRouter,
    RuntimeStatus, UserId, UserIdentity, UserRole,
};

struct RecordingAdapter {
    kind: RuntimeKind,
    executions: Arc<AtomicUsize>,
    deliveries: Arc<AtomicUsize>,
    status: Arc<Mutex<RuntimeStatus>>,
}

impl RecordingAdapter {
    fn new(
        kind: RuntimeKind,
        executions: Arc<AtomicUsize>,
        deliveries: Arc<AtomicUsize>,
        status: Arc<Mutex<RuntimeStatus>>,
    ) -> Self {
        Self {
            kind,
            executions,
            deliveries,
            status,
        }
    }
}

impl RuntimeAdapter for RecordingAdapter {
    fn kind(&self) -> RuntimeKind {
        self.kind
    }

    fn start(
        &self,
        _context: &OperationContext,
        _app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        let mut status = self.status.lock().unwrap();

        *status = RuntimeStatus::Running;

        Ok(*status)
    }

    fn stop(
        &self,
        _context: &OperationContext,
        _app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        let mut status = self.status.lock().unwrap();

        *status = RuntimeStatus::Stopped;

        Ok(*status)
    }

    fn status(
        &self,
        _context: &OperationContext,
        _app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        Ok(*self.status.lock().unwrap())
    }

    fn execute_capability(
        &self,
        app: &InstalledApp,
        execution: &CapabilityExecution,
    ) -> Result<(), RuntimeAdapterError> {
        let identity: Identity = app.identity().clone().into();

        assert_eq!(&identity, execution.provider().identity());

        if self.kind == RuntimeKind::Container {
            let service = app
                .manifest()
                .runtime()
                .entrypoint(&RuntimeEntrypointId::parse("service").unwrap())
                .unwrap();

            assert_eq!(
                service.target().package_path().unwrap().as_str(),
                "runtime/server.oci"
            );
        }

        self.executions.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    fn deliver_event(
        &self,
        app: &InstalledApp,
        delivery: &EventDelivery,
    ) -> Result<(), RuntimeAdapterError> {
        assert_eq!(delivery.subscriber(), &app.identity().clone().into());

        if self.kind == RuntimeKind::Web {
            let main = app
                .manifest()
                .runtime()
                .entrypoint(&RuntimeEntrypointId::parse("main").unwrap())
                .unwrap();

            assert_eq!(
                main.target().package_path().unwrap().as_str(),
                "frontend/index.html"
            );
        }

        self.deliveries.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }
}

fn files_manifest() -> AppManifest {
    let mut runtime = RuntimeDescriptor::container();

    runtime
        .add_entrypoint(RuntimeEntrypoint::container_artifact(
            RuntimeEntrypointId::parse("service").unwrap(),
            PackagePath::parse("runtime/server.oci").unwrap(),
        ))
        .unwrap();

    runtime
        .add_entrypoint(RuntimeEntrypoint::endpoint(
            RuntimeEntrypointId::parse("main").unwrap(),
            RuntimeEndpointId::parse("web").unwrap(),
        ))
        .unwrap();

    let mut manifest = AppManifest::new(
        AppId::parse("com.rumahl.files").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Files",
        runtime,
    )
    .unwrap();

    manifest
        .add_provided_capability(CapabilityId::parse("rumahl.files.preview").unwrap())
        .unwrap();

    manifest
}

fn notes_manifest() -> AppManifest {
    let mut runtime = RuntimeDescriptor::web();

    runtime
        .add_entrypoint(RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        ))
        .unwrap();

    let mut manifest = AppManifest::new(
        AppId::parse("com.rumahl.notes").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Notes",
        runtime,
    )
    .unwrap();

    manifest
        .add_event_subscription(EventName::parse("rumahl.files.changed").unwrap())
        .unwrap();

    manifest
}

fn file(key: &str) -> ResourceRef {
    ResourceRef::new(
        ResourceNamespace::parse("rumahl.files").unwrap(),
        ResourceKind::parse("file").unwrap(),
        ResourceKey::parse(key).unwrap(),
    )
}

#[test]
fn authorized_capability_and_event_delivery_reach_declared_runtimes() {
    let lifecycle = AppLifecycle::new();

    let mut state = PlatformState::new();

    let files = lifecycle.install(files_manifest(), &mut state).unwrap();

    let notes = lifecycle.install(notes_manifest(), &mut state).unwrap();

    let web_deliveries = Arc::new(AtomicUsize::new(0));

    let container_executions = Arc::new(AtomicUsize::new(0));

    let web_status = Arc::new(Mutex::new(RuntimeStatus::Stopped));

    let container_status = Arc::new(Mutex::new(RuntimeStatus::Stopped));

    let mut adapters = RuntimeAdapterRegistry::new();

    adapters
        .register(Box::new(RecordingAdapter::new(
            RuntimeKind::Web,
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&web_deliveries),
            Arc::clone(&web_status),
        )))
        .unwrap();

    adapters
        .register(Box::new(RecordingAdapter::new(
            RuntimeKind::Container,
            Arc::clone(&container_executions),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&container_status),
        )))
        .unwrap();

    let resource = file("document-1");

    let capability = CapabilityId::parse("rumahl.files.preview").unwrap();

    let files_identity = files.identity().clone().into();

    let provider = state
        .capability_registry()
        .provider(&capability, &files_identity)
        .unwrap()
        .clone();

    let mut access_registry = CapabilityAccessRegistry::new();

    let permission = PermissionId::parse("rumahl.files.read").unwrap();

    access_registry
        .register(CapabilityAccessRule::new(capability, permission.clone()))
        .unwrap();

    let notes_identity: Identity = notes.identity().clone().into();

    let user = UserIdentity::new(UserId::new());

    let user_id = *user.id();

    let mut issuer_policy = GrantIssuerPolicy::new();

    issuer_policy.set_user_role(user_id, UserRole::User);

    let grant = GrantAuthority::new()
        .issue(
            &issuer_policy,
            user.into(),
            notes_identity,
            permission,
            PermissionScope::Explicit,
            vec![resource.clone()],
        )
        .unwrap();

    let invocation = CapabilityInvocation::new(
        OperationContext::for_background_app(notes.identity().clone()),
        provider,
    );

    let outcome = CapabilityDispatcher::new()
        .prepare_execution(
            &invocation,
            Some(resource.clone()),
            state.capability_registry(),
            &access_registry,
            &AuthorizationEngine::new(),
            &[grant],
        )
        .unwrap();

    let CapabilityDispatchOutcome::Ready(execution) = outcome else {
        panic!("expected authorized runtime execution");
    };

    RuntimeRouter::new()
        .route_execution(&execution, &state, &adapters)
        .unwrap();

    assert_eq!(container_executions.load(Ordering::Relaxed), 1);
    assert_eq!(web_deliveries.load(Ordering::Relaxed), 0);

    let event = EventEnvelope::new(
        EventName::parse("rumahl.files.changed").unwrap(),
        OperationContext::for_background_app(files.identity().clone()),
        Some(resource),
    );

    let deliveries = state.event_bus().prepare_deliveries(&event);

    assert_eq!(deliveries.len(), 1);

    RuntimeRouter::new()
        .route_event_delivery(&deliveries[0], &state, &adapters)
        .unwrap();

    assert_eq!(web_deliveries.load(Ordering::Relaxed), 1);
    assert_eq!(container_executions.load(Ordering::Relaxed), 1);
}
