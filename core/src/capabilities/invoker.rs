//! One entry point that authorizes a capability invocation and routes the
//! resulting execution to the provider app's runtime.
//!
//! It composes the `CapabilityDispatcher` (identity + permission checks) with
//! the `RuntimeRouter` (adapter selection + delivery), so callers do not wire
//! the two together themselves.
use std::error::Error;
use std::fmt;

use crate::{
    AuthorizationDecision, AuthorizationEngine, CapabilityAccessRegistry, CapabilityInvocation,
    CapabilityRegistry, PermissionGrant, PlatformState, ResourceRef, RuntimeAdapterRegistry,
    RuntimeCapabilityResult, RuntimeRouter, RuntimeRoutingError,
};

use super::{CapabilityDispatchError, CapabilityDispatchOutcome, CapabilityDispatcher};

#[derive(Debug, Default, Clone, Copy)]
pub struct CapabilityInvoker {
    dispatcher: CapabilityDispatcher,
    router: RuntimeRouter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityInvocationOutcome {
    /// Authorized and handed to the provider app's runtime, which returned a result.
    Invoked(RuntimeCapabilityResult),
    NotAuthorized(AuthorizationDecision),
}

#[derive(Debug)]
pub enum CapabilityInvocationError {
    Dispatch(CapabilityDispatchError),
    Routing(RuntimeRoutingError),
}

impl CapabilityInvoker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Authorizes an invocation without delivering it (used when the provider
    /// is reached out of band, for example a web app via the shell).
    pub fn authorize(
        &self,
        invocation: &CapabilityInvocation,
        resource: Option<ResourceRef>,
        capabilities: &CapabilityRegistry,
        access: &CapabilityAccessRegistry,
        grants: &[PermissionGrant],
    ) -> Result<AuthorizationDecision, CapabilityInvocationError> {
        self.dispatcher
            .authorize(
                invocation,
                resource,
                capabilities,
                access,
                &AuthorizationEngine::new(),
                grants,
            )
            .map_err(CapabilityInvocationError::Dispatch)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn invoke(
        &self,
        invocation: &CapabilityInvocation,
        resource: Option<ResourceRef>,
        state: &PlatformState,
        capabilities: &CapabilityRegistry,
        access: &CapabilityAccessRegistry,
        grants: &[PermissionGrant],
        adapters: &RuntimeAdapterRegistry,
    ) -> Result<CapabilityInvocationOutcome, CapabilityInvocationError> {
        let outcome = self
            .dispatcher
            .prepare_execution(
                invocation,
                resource,
                capabilities,
                access,
                &AuthorizationEngine::new(),
                grants,
            )
            .map_err(CapabilityInvocationError::Dispatch)?;

        match outcome {
            CapabilityDispatchOutcome::NotAuthorized(decision) => {
                Ok(CapabilityInvocationOutcome::NotAuthorized(decision))
            }
            CapabilityDispatchOutcome::Ready(execution) => {
                let result = self
                    .router
                    .route_execution(&execution, state, adapters)
                    .map_err(CapabilityInvocationError::Routing)?;
                Ok(CapabilityInvocationOutcome::Invoked(result))
            }
        }
    }
}

impl fmt::Display for CapabilityInvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch(error) => write!(f, "capability dispatch failed: {error}"),
            Self::Routing(error) => write!(f, "capability routing failed: {error}"),
        }
    }
}

impl Error for CapabilityInvocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Dispatch(error) => Some(error),
            Self::Routing(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::{
        AppId, AppIdentity, AppManifest, AppManifestValidator, AppVersion, CapabilityAccessRule,
        CapabilityExecution, CapabilityId, CapabilityProvider, EventDelivery, InstallationId,
        InstalledApp, OperationContext, PackagePath, PermissionId, PermissionScope, PublisherId,
        ResourceKey, ResourceKind, ResourceNamespace, RuntimeAdapter, RuntimeAdapterError,
        RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId, RuntimeKind, RuntimeStatus,
    };

    struct RecordingAdapter {
        kind: RuntimeKind,
        executions: Arc<AtomicUsize>,
    }

    impl RuntimeAdapter for RecordingAdapter {
        fn kind(&self) -> RuntimeKind {
            self.kind
        }
        fn start(
            &self,
            _: &OperationContext,
            _: &InstalledApp,
        ) -> Result<RuntimeStatus, RuntimeAdapterError> {
            Ok(RuntimeStatus::Running)
        }
        fn stop(
            &self,
            _: &OperationContext,
            _: &InstalledApp,
        ) -> Result<RuntimeStatus, RuntimeAdapterError> {
            Ok(RuntimeStatus::Stopped)
        }
        fn status(
            &self,
            _: &OperationContext,
            _: &InstalledApp,
        ) -> Result<RuntimeStatus, RuntimeAdapterError> {
            Ok(RuntimeStatus::Running)
        }
        fn execute_capability(
            &self,
            _: &InstalledApp,
            _: &CapabilityExecution,
        ) -> Result<RuntimeCapabilityResult, RuntimeAdapterError> {
            self.executions.fetch_add(1, Ordering::Relaxed);
            Ok(RuntimeCapabilityResult::new("{\"preview\":\"ok\"}".to_owned()))
        }
        fn deliver_event(
            &self,
            _: &InstalledApp,
            _: &EventDelivery,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }
    }

    fn container_app(id: &str) -> InstalledApp {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse(id).unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Service",
            runtime,
        )
        .unwrap();
        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    fn file(key: &str) -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse(key).unwrap(),
        )
    }

    #[test]
    fn authorizes_and_routes_a_capability_invocation() {
        let provider_app = container_app("com.rumahl.files");
        let consumer = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );
        let capability = CapabilityId::parse("rumahl.files.preview").unwrap();
        let permission = PermissionId::parse("rumahl.files.preview").unwrap();
        let resource = file("document-1");

        let mut state = PlatformState::new();
        state.installed_apps_mut().register(provider_app.clone()).unwrap();

        let mut capabilities = CapabilityRegistry::new();
        capabilities
            .register(
                CapabilityProvider::new(provider_app.identity().clone().into(), capability.clone())
                    .unwrap(),
            )
            .unwrap();
        let mut access = CapabilityAccessRegistry::new();
        access
            .register(CapabilityAccessRule::new(capability.clone(), permission.clone()))
            .unwrap();

        // The consumer app is explicitly granted the permission for this resource.
        let grant = PermissionGrant::new(
            consumer.clone().into(),
            permission,
            PermissionScope::Explicit,
            vec![resource.clone()],
            consumer.clone().into(),
        )
        .unwrap();

        let executions = Arc::new(AtomicUsize::new(0));
        let mut adapters = RuntimeAdapterRegistry::new();
        adapters
            .register(Box::new(RecordingAdapter {
                kind: RuntimeKind::Container,
                executions: Arc::clone(&executions),
            }))
            .unwrap();

        let invocation = CapabilityInvocation::new(
            OperationContext::for_background_app(consumer.clone()),
            CapabilityProvider::new(provider_app.identity().clone().into(), capability).unwrap(),
        );

        let outcome = CapabilityInvoker::new()
            .invoke(
                &invocation,
                Some(resource),
                &state,
                &capabilities,
                &access,
                &[grant],
                &adapters,
            )
            .unwrap();
        assert_eq!(
            outcome,
            CapabilityInvocationOutcome::Invoked(RuntimeCapabilityResult::new(
                "{\"preview\":\"ok\"}".to_owned()
            ))
        );
        assert_eq!(executions.load(Ordering::Relaxed), 1);
    }
}
