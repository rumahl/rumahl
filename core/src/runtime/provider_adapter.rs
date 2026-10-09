//! Bridges the runtime router to the OS runtime-supervisor boundary.
//!
//! `AppRuntimeProvider` models installation-scoped runtime lifecycles
//! (prepare/activate/deactivate/remove). This adapter exposes that boundary as
//! a `RuntimeAdapter` so `RuntimeRouter::start_app`/`stop_app`/`app_status`
//! reach the real runtime instead of failing closed.
use crate::{AppRuntimeInstallationState, AppRuntimeProvider, InstalledApp, OperationContext};

use super::{RuntimeAdapter, RuntimeAdapterError, RuntimeChannelRegistry, RuntimeKind, RuntimeStatus};

/// Adapts one `AppRuntimeProvider` for one `RuntimeKind`.
pub struct ProviderRuntimeAdapter<P> {
    kind: RuntimeKind,
    provider: P,
    /// Live channels for background event delivery. `None` fails closed.
    channels: Option<std::sync::Arc<RuntimeChannelRegistry>>,
}

impl<P> ProviderRuntimeAdapter<P> {
    pub fn new(kind: RuntimeKind, provider: P) -> Self {
        Self {
            kind,
            provider,
            channels: None,
        }
    }

    /// Enables OS -> app event delivery over live runtime channels.
    pub fn with_channels(mut self, channels: std::sync::Arc<RuntimeChannelRegistry>) -> Self {
        self.channels = Some(channels);
        self
    }
}

impl<P> RuntimeAdapter for ProviderRuntimeAdapter<P>
where
    P: AppRuntimeProvider + Send + Sync,
{
    fn kind(&self) -> RuntimeKind {
        self.kind
    }

    fn start(
        &self,
        _context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        // Activation is idempotent, so replaying it is safe.
        self.provider
            .activate_installation(app)
            .map_err(|_| RuntimeAdapterError::Failed)?;
        Ok(RuntimeStatus::Running)
    }

    fn stop(
        &self,
        _context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        self.provider
            .deactivate_installation(app.installation_id())
            .map_err(|_| RuntimeAdapterError::Failed)?;
        Ok(RuntimeStatus::Stopped)
    }

    fn status(
        &self,
        _context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError> {
        let state = self
            .provider
            .installation_state(app.installation_id())
            .map_err(|_| RuntimeAdapterError::Failed)?;
        Ok(match state {
            AppRuntimeInstallationState::Active => RuntimeStatus::Running,
            AppRuntimeInstallationState::Prepared | AppRuntimeInstallationState::Absent => {
                RuntimeStatus::Stopped
            }
        })
    }

    fn execute_capability(
        &self,
        _app: &InstalledApp,
        _execution: &crate::CapabilityExecution,
    ) -> Result<(), RuntimeAdapterError> {
        // Capability routing to an app runtime is not part of the supervisor
        // installation boundary yet.
        Err(RuntimeAdapterError::Unavailable)
    }

    fn deliver_event(
        &self,
        _app: &InstalledApp,
        delivery: &crate::EventDelivery,
    ) -> Result<(), RuntimeAdapterError> {
        // Deliver over the app's live channel so background services receive
        // events without an open window.
        let channels = self
            .channels
            .as_ref()
            .ok_or(RuntimeAdapterError::Unavailable)?;
        channels
            .deliver_event(delivery)
            .map_err(|_| RuntimeAdapterError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, InstallationId, PackagePath,
        PublisherId, RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    struct FakeProvider {
        active: Mutex<bool>,
    }

    impl AppRuntimeProvider for FakeProvider {
        type Error = std::io::Error;

        fn prepare_installation(&self, _app: &InstalledApp) -> Result<(), Self::Error> {
            Ok(())
        }

        fn activate_installation(&self, _app: &InstalledApp) -> Result<(), Self::Error> {
            *self.active.lock().unwrap() = true;
            Ok(())
        }

        fn installation_state(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<AppRuntimeInstallationState, Self::Error> {
            Ok(if *self.active.lock().unwrap() {
                AppRuntimeInstallationState::Active
            } else {
                AppRuntimeInstallationState::Absent
            })
        }

        fn deactivate_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            *self.active.lock().unwrap() = false;
            Ok(true)
        }

        fn remove_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    fn app() -> InstalledApp {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("image.tar").unwrap(),
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
    fn maps_lifecycle_to_supervisor_state() {
        let adapter = ProviderRuntimeAdapter::new(
            RuntimeKind::Container,
            FakeProvider {
                active: Mutex::new(false),
            },
        );
        let app = app();
        let context = OperationContext::for_background_app(app.identity().clone());

        assert_eq!(adapter.kind(), RuntimeKind::Container);
        assert_eq!(adapter.status(&context, &app).unwrap(), RuntimeStatus::Stopped);
        assert_eq!(adapter.start(&context, &app).unwrap(), RuntimeStatus::Running);
        assert_eq!(adapter.status(&context, &app).unwrap(), RuntimeStatus::Running);
        assert_eq!(adapter.stop(&context, &app).unwrap(), RuntimeStatus::Stopped);
        assert_eq!(adapter.status(&context, &app).unwrap(), RuntimeStatus::Stopped);
    }

    #[test]
    fn delivers_events_over_a_registered_channel() {
        use std::sync::Arc;

        use crate::{
            EventDelivery, EventEnvelope, EventName, RuntimeChannel, RuntimeChannelError,
            RuntimeEvent,
        };

        #[derive(Default)]
        struct Sink(std::sync::Mutex<Vec<String>>);

        impl RuntimeChannel for Sink {
            fn deliver(&self, event: &RuntimeEvent) -> Result<(), RuntimeChannelError> {
                self.0.lock().unwrap().push(event.topic.clone());
                Ok(())
            }
        }

        let app = app();
        let channels = Arc::new(RuntimeChannelRegistry::new());
        let sink = Arc::new(Sink::default());
        channels.register(*app.installation_id(), sink.clone());
        let adapter = ProviderRuntimeAdapter::new(
            RuntimeKind::Container,
            FakeProvider {
                active: Mutex::new(true),
            },
        )
        .with_channels(channels);

        let event = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(app.identity().clone()),
            None,
        );
        adapter
            .deliver_event(&app, &EventDelivery::new(app.identity().clone().into(), event))
            .unwrap();
        assert_eq!(sink.0.lock().unwrap().as_slice(), ["rumahl.files.changed"]);

        // Without a channel registry the adapter fails closed.
        let bare = ProviderRuntimeAdapter::new(
            RuntimeKind::Container,
            FakeProvider {
                active: Mutex::new(false),
            },
        );
        let event = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(app.identity().clone()),
            None,
        );
        assert_eq!(
            bare.deliver_event(&app, &EventDelivery::new(app.identity().clone().into(), event)),
            Err(RuntimeAdapterError::Unavailable)
        );
    }
}
