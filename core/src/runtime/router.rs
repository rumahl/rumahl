use std::error::Error;
use std::fmt;

use crate::{CapabilityExecution, EventDelivery, Identity, InstalledApp, PlatformState};

use super::{RuntimeAdapterError, RuntimeAdapterRegistry, RuntimeKind};

#[derive(Debug, Default, Clone, Copy)]
pub struct RuntimeRouter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRoutingError {
    TargetIsNotApp,
    AppNotInstalled,
    AdapterNotRegistered(RuntimeKind),
    AdapterFailed(RuntimeAdapterError),
}

impl RuntimeRouter {
    pub fn new() -> Self {
        Self
    }

    pub fn route_execution(
        &self,
        execution: &CapabilityExecution,
        state: &PlatformState,
        adapters: &RuntimeAdapterRegistry,
    ) -> Result<(), RuntimeRoutingError> {
        let app = self.resolve_app(execution.provider().identity(), state)?;

        let adapter = self.resolve_adapter(app, adapters)?;

        adapter
            .execute_capability(app, execution)
            .map_err(RuntimeRoutingError::AdapterFailed)
    }

    pub fn route_event_delivery(
        &self,
        delivery: &EventDelivery,
        state: &PlatformState,
        adapters: &RuntimeAdapterRegistry,
    ) -> Result<(), RuntimeRoutingError> {
        let app = self.resolve_app(delivery.subscriber(), state)?;

        let adapter = self.resolve_adapter(app, adapters)?;

        adapter
            .deliver_event(app, delivery)
            .map_err(RuntimeRoutingError::AdapterFailed)
    }

    fn resolve_app<'a>(
        &self,
        target: &Identity,
        state: &'a PlatformState,
    ) -> Result<&'a InstalledApp, RuntimeRoutingError> {
        let Identity::App(identity) = target else {
            return Err(RuntimeRoutingError::TargetIsNotApp);
        };

        state
            .installed_apps()
            .get_by_installation_id(identity.installation_id())
            .filter(|app| app.identity() == identity)
            .ok_or(RuntimeRoutingError::AppNotInstalled)
    }

    fn resolve_adapter<'a>(
        &self,
        app: &InstalledApp,
        adapters: &'a RuntimeAdapterRegistry,
    ) -> Result<&'a dyn super::RuntimeAdapter, RuntimeRoutingError> {
        let kind = app.manifest().runtime().kind();

        adapters
            .adapter(kind)
            .ok_or(RuntimeRoutingError::AdapterNotRegistered(kind))
    }
}

impl fmt::Display for RuntimeRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetIsNotApp => {
                write!(f, "runtime target is not an app")
            }

            Self::AppNotInstalled => {
                write!(f, "runtime target app is not installed")
            }

            Self::AdapterNotRegistered(kind) => {
                write!(f, "no runtime adapter is registered for '{kind:?}'")
            }

            Self::AdapterFailed(error) => {
                write!(f, "runtime adapter failed: {error}")
            }
        }
    }
}

impl Error for RuntimeRoutingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AdapterFailed(error) => Some(error),

            Self::TargetIsNotApp | Self::AppNotInstalled | Self::AdapterNotRegistered(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, CapabilityId, CapabilityProvider,
        EventEnvelope, EventName, InstallationId, OperationContext, PublisherId, RuntimeAdapter,
        RuntimeDescriptor, ServiceId, ServiceIdentity,
    };

    struct RecordingAdapter {
        kind: RuntimeKind,
        executions: Arc<AtomicUsize>,
        deliveries: Arc<AtomicUsize>,
        failure: Option<RuntimeAdapterError>,
    }

    impl RecordingAdapter {
        fn new(
            kind: RuntimeKind,
            executions: Arc<AtomicUsize>,
            deliveries: Arc<AtomicUsize>,
        ) -> Self {
            Self {
                kind,
                executions,
                deliveries,
                failure: None,
            }
        }

        fn failing(kind: RuntimeKind, failure: RuntimeAdapterError) -> Self {
            Self {
                kind,
                executions: Arc::new(AtomicUsize::new(0)),
                deliveries: Arc::new(AtomicUsize::new(0)),
                failure: Some(failure),
            }
        }
    }

    impl RuntimeAdapter for RecordingAdapter {
        fn kind(&self) -> RuntimeKind {
            self.kind
        }

        fn execute_capability(
            &self,
            _app: &InstalledApp,
            _execution: &CapabilityExecution,
        ) -> Result<(), RuntimeAdapterError> {
            if let Some(error) = self.failure {
                return Err(error);
            }

            self.executions.fetch_add(1, Ordering::Relaxed);

            Ok(())
        }

        fn deliver_event(
            &self,
            _app: &InstalledApp,
            _delivery: &EventDelivery,
        ) -> Result<(), RuntimeAdapterError> {
            if let Some(error) = self.failure {
                return Err(error);
            }

            self.deliveries.fetch_add(1, Ordering::Relaxed);

            Ok(())
        }
    }

    fn installed_app(kind: RuntimeKind) -> InstalledApp {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            RuntimeDescriptor::new(kind),
        )
        .unwrap();

        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    fn state_with(app: InstalledApp) -> PlatformState {
        let mut state = PlatformState::new();

        state.installed_apps_mut().register(app).unwrap();

        state
    }

    fn execution_for(app: &InstalledApp) -> CapabilityExecution {
        let provider = CapabilityProvider::new(
            app.identity().clone().into(),
            CapabilityId::parse("com.rumahl.notes.create").unwrap(),
        )
        .unwrap();

        CapabilityExecution::new(
            OperationContext::for_background_app(app.identity().clone()),
            provider,
            None,
        )
    }

    fn delivery_for(app: &InstalledApp) -> EventDelivery {
        let event = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(app.identity().clone()),
            None,
        );

        EventDelivery::new(app.identity().clone().into(), event)
    }

    #[test]
    fn routes_capability_execution_to_matching_adapter() {
        let app = installed_app(RuntimeKind::Web);

        let execution = execution_for(&app);

        let state = state_with(app);

        let executions = Arc::new(AtomicUsize::new(0));

        let mut adapters = RuntimeAdapterRegistry::new();

        adapters
            .register(Box::new(RecordingAdapter::new(
                RuntimeKind::Web,
                Arc::clone(&executions),
                Arc::new(AtomicUsize::new(0)),
            )))
            .unwrap();

        RuntimeRouter::new()
            .route_execution(&execution, &state, &adapters)
            .unwrap();

        assert_eq!(executions.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn routes_event_delivery_to_matching_adapter() {
        let app = installed_app(RuntimeKind::Container);

        let delivery = delivery_for(&app);

        let state = state_with(app);

        let deliveries = Arc::new(AtomicUsize::new(0));

        let mut adapters = RuntimeAdapterRegistry::new();

        adapters
            .register(Box::new(RecordingAdapter::new(
                RuntimeKind::Container,
                Arc::new(AtomicUsize::new(0)),
                Arc::clone(&deliveries),
            )))
            .unwrap();

        RuntimeRouter::new()
            .route_event_delivery(&delivery, &state, &adapters)
            .unwrap();

        assert_eq!(deliveries.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn rejects_route_without_matching_adapter() {
        let app = installed_app(RuntimeKind::Native);

        let execution = execution_for(&app);

        let state = state_with(app);

        let result = RuntimeRouter::new().route_execution(
            &execution,
            &state,
            &RuntimeAdapterRegistry::new(),
        );

        assert_eq!(
            result.unwrap_err(),
            RuntimeRoutingError::AdapterNotRegistered(RuntimeKind::Native)
        );
    }

    #[test]
    fn rejects_route_to_uninstalled_app() {
        let app = installed_app(RuntimeKind::Web);

        let execution = execution_for(&app);

        let result = RuntimeRouter::new().route_execution(
            &execution,
            &PlatformState::new(),
            &RuntimeAdapterRegistry::new(),
        );

        assert_eq!(result.unwrap_err(), RuntimeRoutingError::AppNotInstalled);
    }

    #[test]
    fn rejects_non_app_runtime_target() {
        let service = ServiceIdentity::new(ServiceId::parse("rumahl.storage").unwrap());

        let provider = CapabilityProvider::new(
            service.into(),
            CapabilityId::parse("rumahl.storage.read").unwrap(),
        )
        .unwrap();

        let actor = crate::AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let execution =
            CapabilityExecution::new(OperationContext::for_background_app(actor), provider, None);

        let result = RuntimeRouter::new().route_execution(
            &execution,
            &PlatformState::new(),
            &RuntimeAdapterRegistry::new(),
        );

        assert_eq!(result.unwrap_err(), RuntimeRoutingError::TargetIsNotApp);
    }

    #[test]
    fn propagates_adapter_failure() {
        let app = installed_app(RuntimeKind::Web);

        let execution = execution_for(&app);

        let state = state_with(app);

        let mut adapters = RuntimeAdapterRegistry::new();

        adapters
            .register(Box::new(RecordingAdapter::failing(
                RuntimeKind::Web,
                RuntimeAdapterError::Unavailable,
            )))
            .unwrap();

        let result = RuntimeRouter::new().route_execution(&execution, &state, &adapters);

        assert_eq!(
            result.unwrap_err(),
            RuntimeRoutingError::AdapterFailed(RuntimeAdapterError::Unavailable)
        );
    }
}
