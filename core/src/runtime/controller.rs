//! Installation-scoped runtime control built on the router and adapters.
//!
//! `RuntimeController` is the shared entry point for the platform service: it
//! resolves an installed app from recovered state, builds a background
//! `OperationContext` for it, and routes status/start/stop to the registered
//! `RuntimeAdapter` (which the supervisor-backed adapter implements).
use std::sync::Arc;

use crate::{InstallationId, InstalledApp, OperationContext, PlatformState};

use super::{RuntimeAdapterRegistry, RuntimeRouter, RuntimeRoutingError, RuntimeStatus};

pub struct RuntimeController {
    router: RuntimeRouter,
    state: Arc<PlatformState>,
    adapters: Arc<RuntimeAdapterRegistry>,
}

impl RuntimeController {
    pub fn new(state: Arc<PlatformState>, adapters: Arc<RuntimeAdapterRegistry>) -> Self {
        Self {
            router: RuntimeRouter::new(),
            state,
            adapters,
        }
    }

    pub fn status(
        &self,
        installation_id: &InstallationId,
    ) -> Result<RuntimeStatus, RuntimeRoutingError> {
        let context = self.context(installation_id)?;
        self.router
            .app_status(&context, installation_id, &self.state, &self.adapters)
    }

    pub fn start(
        &self,
        installation_id: &InstallationId,
    ) -> Result<RuntimeStatus, RuntimeRoutingError> {
        let context = self.context(installation_id)?;
        self.router
            .start_app(&context, installation_id, &self.state, &self.adapters)
    }

    pub fn stop(
        &self,
        installation_id: &InstallationId,
    ) -> Result<RuntimeStatus, RuntimeRoutingError> {
        let context = self.context(installation_id)?;
        self.router
            .stop_app(&context, installation_id, &self.state, &self.adapters)
    }

    fn context(
        &self,
        installation_id: &InstallationId,
    ) -> Result<OperationContext, RuntimeRoutingError> {
        Ok(OperationContext::for_background_app(
            self.app(installation_id)?.identity().clone(),
        ))
    }

    fn app(&self, installation_id: &InstallationId) -> Result<&InstalledApp, RuntimeRoutingError> {
        self.state
            .installed_apps()
            .get_by_installation_id(installation_id)
            .ok_or(RuntimeRoutingError::AppNotInstalled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, CapabilityExecution, EventDelivery,
        PackagePath, PublisherId, RuntimeAdapter, RuntimeAdapterError, RuntimeDescriptor,
        RuntimeEntrypoint, RuntimeEntrypointId, RuntimeKind,
    };

    struct StubAdapter {
        kind: RuntimeKind,
        status: Mutex<RuntimeStatus>,
    }

    impl StubAdapter {
        fn new(kind: RuntimeKind) -> Self {
            Self {
                kind,
                status: Mutex::new(RuntimeStatus::Stopped),
            }
        }
    }

    impl RuntimeAdapter for StubAdapter {
        fn kind(&self) -> RuntimeKind {
            self.kind
        }

        fn start(
            &self,
            _context: &OperationContext,
            _app: &InstalledApp,
        ) -> Result<RuntimeStatus, RuntimeAdapterError> {
            *self.status.lock().unwrap() = RuntimeStatus::Running;
            Ok(RuntimeStatus::Running)
        }

        fn stop(
            &self,
            _context: &OperationContext,
            _app: &InstalledApp,
        ) -> Result<RuntimeStatus, RuntimeAdapterError> {
            *self.status.lock().unwrap() = RuntimeStatus::Stopped;
            Ok(RuntimeStatus::Stopped)
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
            _app: &InstalledApp,
            _execution: &CapabilityExecution,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn deliver_event(
            &self,
            _app: &InstalledApp,
            _delivery: &EventDelivery,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }
    }

    fn app() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.cloud").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Cloud",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn controls_status_start_stop_by_installation() {
        let app = app();
        let installation = *app.installation_id();
        let mut state = PlatformState::new();
        state.installed_apps_mut().register(app).unwrap();
        let mut adapters = RuntimeAdapterRegistry::new();
        adapters
            .register(Box::new(StubAdapter::new(RuntimeKind::Container)))
            .unwrap();

        let controller = RuntimeController::new(Arc::new(state), Arc::new(adapters));

        assert_eq!(controller.status(&installation).unwrap(), RuntimeStatus::Stopped);
        assert_eq!(controller.start(&installation).unwrap(), RuntimeStatus::Running);
        assert_eq!(controller.status(&installation).unwrap(), RuntimeStatus::Running);
        assert_eq!(controller.stop(&installation).unwrap(), RuntimeStatus::Stopped);
    }

    #[test]
    fn unknown_installation_fails_closed() {
        let controller = RuntimeController::new(
            Arc::new(PlatformState::new()),
            Arc::new(RuntimeAdapterRegistry::new()),
        );

        assert_eq!(
            controller.status(&InstallationId::new()),
            Err(RuntimeRoutingError::AppNotInstalled)
        );
    }
}
