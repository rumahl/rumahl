use std::error::Error;
use std::fmt;

use rumahl_core::{
    AppDatabaseProvider, AppLifecycle, AppLifecycleError, AppManifest, AppOperation,
    AppOperationError, AppOperationId, AppOperationKind, AppOperationPhase, AppOperationRepository,
    AppOperationResource, AppOperationResourceState, InMemoryGrantStore, InstalledApp,
    PlatformSnapshot, PlatformSnapshotRepository, PlatformState, SecretStore, UnixTimestamp,
    UnixTimestampError,
};
use rumahl_oidc_provider::{
    InstalledAppOriginResolver, OidcClientProvisioningError, OidcClientRegistrar,
    OidcClientRepository,
};

use crate::RuntimeSecretDelivery;

type BoxedError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct AppInstallResult {
    operation_id: AppOperationId,
    app: InstalledApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppOperationRecoveryReport {
    resumed: usize,
    committed: usize,
}

#[derive(Debug)]
pub enum AppOperationRunnerError {
    Clock(UnixTimestampError),
    AppLifecycle(AppLifecycleError),
    Operation(AppOperationError),
    Journal(BoxedError),
    DatabaseProvider(BoxedError),
    OidcProvisioning(BoxedError),
    SnapshotRepository(BoxedError),
    RuntimeSecretDelivery(BoxedError),
    MissingOperationTarget(AppOperationId),
    UnsupportedOperationKind(AppOperationKind),
    UnsupportedOperationPhase(AppOperationPhase),
    UnexpectedResourceState {
        resource: AppOperationResource,
        state: AppOperationResourceState,
    },
    RecoveredTargetMismatch(AppOperationId),
}

/// Executes and resumes durable app installation plans.
///
/// Every participant call is bracketed by journal transitions. A participant
/// left in `applying` is replayed after restart, so providers and runtime
/// delivery must honor their idempotency contracts. The platform snapshot is
/// the final resource and live in-memory state is published only after the
/// operation has been durably committed.
pub struct AppOperationRunner<J, D, R, O, P, S, T> {
    journal: J,
    database_provider: D,
    oidc_registrar: OidcClientRegistrar<R, O>,
    snapshot_repository: P,
    secret_store: S,
    runtime_secrets: T,
    lifecycle: AppLifecycle,
}

impl<J, D, R, O, P, S, T> AppOperationRunner<J, D, R, O, P, S, T>
where
    J: AppOperationRepository,
    D: AppDatabaseProvider,
    R: OidcClientRepository,
    O: InstalledAppOriginResolver,
    P: PlatformSnapshotRepository,
    S: SecretStore,
    T: RuntimeSecretDelivery,
{
    pub fn new(
        journal: J,
        database_provider: D,
        oidc_repository: R,
        origin_resolver: O,
        snapshot_repository: P,
        secret_store: S,
        runtime_secrets: T,
    ) -> Self {
        Self {
            journal,
            database_provider,
            oidc_registrar: OidcClientRegistrar::new(oidc_repository, origin_resolver),
            snapshot_repository,
            secret_store,
            runtime_secrets,
            lifecycle: AppLifecycle::new(),
        }
    }

    pub fn install(
        &self,
        manifest: AppManifest,
        state: &mut PlatformState,
        grant_store: &InMemoryGrantStore,
    ) -> Result<AppInstallResult, AppOperationRunnerError> {
        let mut staged = state.clone();
        let app = self
            .lifecycle
            .install(manifest, &mut staged)
            .map_err(AppOperationRunnerError::AppLifecycle)?;
        let started_at = UnixTimestamp::now().map_err(AppOperationRunnerError::Clock)?;
        let operation = AppOperation::for_app(&app, AppOperationKind::Install, started_at)
            .map_err(AppOperationRunnerError::Operation)?;
        let operation_id = *operation.id();

        self.journal
            .create(&operation)
            .map_err(|error| AppOperationRunnerError::Journal(Box::new(error)))?;
        self.apply_install(operation, staged, state, grant_store)?;

        Ok(AppInstallResult { operation_id, app })
    }

    pub fn recover_incomplete(
        &self,
        state: &mut PlatformState,
        grant_store: &InMemoryGrantStore,
    ) -> Result<AppOperationRecoveryReport, AppOperationRunnerError> {
        let operations = self
            .journal
            .list_incomplete()
            .map_err(|error| AppOperationRunnerError::Journal(Box::new(error)))?;
        let resumed = operations.len();
        let mut committed = 0;

        for operation in operations {
            if operation.kind() != AppOperationKind::Install {
                return Err(AppOperationRunnerError::UnsupportedOperationKind(
                    operation.kind(),
                ));
            }
            if operation.phase() != AppOperationPhase::Applying {
                return Err(AppOperationRunnerError::UnsupportedOperationPhase(
                    operation.phase(),
                ));
            }

            let target = operation
                .target_app()
                .ok_or_else(|| AppOperationRunnerError::MissingOperationTarget(*operation.id()))?;
            let app = target.installed_app();
            let mut staged = state.clone();

            match staged
                .installed_apps()
                .get_by_installation_id(app.installation_id())
            {
                Some(existing) if existing == app => {}
                Some(_) => {
                    return Err(AppOperationRunnerError::RecoveredTargetMismatch(
                        *operation.id(),
                    ));
                }
                None => {
                    self.lifecycle
                        .restore(app.clone(), &mut staged)
                        .map_err(AppOperationRunnerError::AppLifecycle)?;
                }
            }

            self.apply_install(operation, staged, state, grant_store)?;
            committed += 1;
        }

        Ok(AppOperationRecoveryReport { resumed, committed })
    }

    fn apply_install(
        &self,
        mut operation: AppOperation,
        staged: PlatformState,
        live: &mut PlatformState,
        grant_store: &InMemoryGrantStore,
    ) -> Result<(), AppOperationRunnerError> {
        let app = operation
            .target_app()
            .ok_or_else(|| AppOperationRunnerError::MissingOperationTarget(*operation.id()))?
            .installed_app()
            .clone();
        let steps = operation.steps().to_vec();

        for step in steps {
            match step.state() {
                AppOperationResourceState::Applied => continue,
                AppOperationResourceState::Pending => {
                    let at = transition_time(&operation)?;
                    operation
                        .begin_resource(step.resource(), at)
                        .map_err(AppOperationRunnerError::Operation)?;
                    self.store_transition(&operation)?;
                }
                AppOperationResourceState::Applying => {}
                state => {
                    return Err(AppOperationRunnerError::UnexpectedResourceState {
                        resource: step.resource(),
                        state,
                    });
                }
            }

            self.apply_resource(&operation, step.resource(), &app, &staged, grant_store)?;

            let at = transition_time(&operation)?;
            operation
                .complete_resource(step.resource(), at)
                .map_err(AppOperationRunnerError::Operation)?;
            self.store_transition(&operation)?;
        }

        let at = transition_time(&operation)?;
        operation
            .commit(at)
            .map_err(AppOperationRunnerError::Operation)?;
        self.store_transition(&operation)?;
        *live = staged;

        Ok(())
    }

    fn apply_resource(
        &self,
        operation: &AppOperation,
        resource: AppOperationResource,
        app: &InstalledApp,
        staged: &PlatformState,
        grant_store: &InMemoryGrantStore,
    ) -> Result<(), AppOperationRunnerError> {
        match resource {
            AppOperationResource::AppDatabases => {
                let bindings = staged
                    .database_registry()
                    .databases_for_owner(app.identity())
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                self.database_provider
                    .provision_installation(&bindings)
                    .map_err(|error| AppOperationRunnerError::DatabaseProvider(Box::new(error)))
            }
            AppOperationResource::OidcClient => {
                let registration = self
                    .oidc_registrar
                    .register_or_recover_installed_app(
                        staged,
                        app.installation_id(),
                        operation.started_at(),
                        &self.secret_store,
                    )
                    .map_err(|error: OidcClientProvisioningError<_, _, _>| {
                        AppOperationRunnerError::OidcProvisioning(Box::new(error))
                    })?;

                if let Some(registration) = registration
                    && let Some(secret) = registration.client_secret()
                {
                    self.runtime_secrets
                        .deliver_oidc_client_secret(
                            operation.id(),
                            app,
                            registration.client().client_id(),
                            secret,
                        )
                        .map_err(|error| {
                            AppOperationRunnerError::RuntimeSecretDelivery(Box::new(error))
                        })?;
                }

                Ok(())
            }
            AppOperationResource::PlatformSnapshot => self
                .snapshot_repository
                .store(&PlatformSnapshot::capture(staged, grant_store))
                .map_err(|error| AppOperationRunnerError::SnapshotRepository(Box::new(error))),
        }
    }

    fn store_transition(&self, operation: &AppOperation) -> Result<(), AppOperationRunnerError> {
        self.journal
            .store_transition(operation)
            .map_err(|error| AppOperationRunnerError::Journal(Box::new(error)))
    }

    pub fn journal(&self) -> &J {
        &self.journal
    }

    pub fn database_provider(&self) -> &D {
        &self.database_provider
    }

    pub fn oidc_repository(&self) -> &R {
        self.oidc_registrar.repository()
    }

    pub fn snapshot_repository(&self) -> &P {
        &self.snapshot_repository
    }

    pub fn secret_store(&self) -> &S {
        &self.secret_store
    }

    pub fn runtime_secrets(&self) -> &T {
        &self.runtime_secrets
    }
}

impl AppInstallResult {
    pub fn operation_id(&self) -> &AppOperationId {
        &self.operation_id
    }

    pub fn app(&self) -> &InstalledApp {
        &self.app
    }
}

impl AppOperationRecoveryReport {
    pub fn resumed(&self) -> usize {
        self.resumed
    }

    pub fn committed(&self) -> usize {
        self.committed
    }
}

fn transition_time(operation: &AppOperation) -> Result<UnixTimestamp, AppOperationRunnerError> {
    let now = UnixTimestamp::now().map_err(AppOperationRunnerError::Clock)?;
    Ok(std::cmp::max(now, operation.updated_at()))
}

impl fmt::Display for AppOperationRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => write!(f, "app operation clock failed: {error}"),
            Self::AppLifecycle(error) => write!(f, "app lifecycle failed: {error}"),
            Self::Operation(error) => write!(f, "app operation transition failed: {error}"),
            Self::Journal(_) => write!(f, "app operation journal failed"),
            Self::DatabaseProvider(_) => write!(f, "app database provisioning failed"),
            Self::OidcProvisioning(_) => write!(f, "OIDC client provisioning failed"),
            Self::SnapshotRepository(_) => write!(f, "platform snapshot persistence failed"),
            Self::RuntimeSecretDelivery(_) => write!(f, "runtime secret delivery failed"),
            Self::MissingOperationTarget(id) => {
                write!(f, "app operation {id} has no durable installation target")
            }
            Self::UnsupportedOperationKind(kind) => {
                write!(
                    f,
                    "app operation kind {kind:?} is not supported by the install runner"
                )
            }
            Self::UnsupportedOperationPhase(phase) => {
                write!(
                    f,
                    "app operation phase {phase:?} is not supported by the install runner"
                )
            }
            Self::UnexpectedResourceState { resource, state } => write!(
                f,
                "app operation resource {resource:?} has unexpected state {state:?}"
            ),
            Self::RecoveredTargetMismatch(id) => {
                write!(
                    f,
                    "recovered app operation {id} conflicts with live platform state"
                )
            }
        }
    }
}

impl Error for AppOperationRunnerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::AppLifecycle(error) => Some(error),
            Self::Operation(error) => Some(error),
            Self::Journal(error)
            | Self::DatabaseProvider(error)
            | Self::OidcProvisioning(error)
            | Self::SnapshotRepository(error)
            | Self::RuntimeSecretDelivery(error) => Some(error.as_ref()),
            Self::MissingOperationTarget(_)
            | Self::UnsupportedOperationKind(_)
            | Self::UnsupportedOperationPhase(_)
            | Self::UnexpectedResourceState { .. }
            | Self::RecoveredTargetMismatch(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rumahl_core::{
        AppDatabaseBinding, AppDatabaseDeclaration, AppDatabaseId, AppDatabaseInstallationState,
        AppId, AppOperationStep, AppVersion, InstallationId, OidcCallbackPath,
        OidcClientDeclaration, OidcClientType, OidcScope, PackagePath, PublisherId,
        RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint, RuntimeEntrypointId, SecretId,
        SecretPurpose, SecretRecord, SecretValue,
    };
    use rumahl_oidc_provider::{
        OidcClientId, OidcClientRecord, OidcClientSecret, OidcClientSecretDigest,
    };
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestError(&'static str);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    #[derive(Default)]
    struct MemoryJournal {
        operations: RefCell<Vec<AppOperation>>,
    }

    impl AppOperationRepository for MemoryJournal {
        type Error = TestError;

        fn create(&self, operation: &AppOperation) -> Result<(), Self::Error> {
            if self
                .operations
                .borrow()
                .iter()
                .any(|stored| stored.id() == operation.id())
            {
                return Err(TestError("duplicate operation"));
            }
            self.operations.borrow_mut().push(operation.clone());
            Ok(())
        }

        fn store_transition(&self, operation: &AppOperation) -> Result<(), Self::Error> {
            let mut operations = self.operations.borrow_mut();
            let stored = operations
                .iter_mut()
                .find(|stored| stored.id() == operation.id())
                .ok_or(TestError("missing operation"))?;
            if stored.revision().checked_add(1) != Some(operation.revision()) {
                return Err(TestError("stale transition"));
            }
            *stored = operation.clone();
            Ok(())
        }

        fn find(&self, id: &AppOperationId) -> Result<Option<AppOperation>, Self::Error> {
            Ok(self
                .operations
                .borrow()
                .iter()
                .find(|operation| operation.id() == id)
                .cloned())
        }

        fn list_incomplete(&self) -> Result<Vec<AppOperation>, Self::Error> {
            Ok(self
                .operations
                .borrow()
                .iter()
                .filter(|operation| !operation.is_terminal())
                .cloned()
                .collect())
        }
    }

    #[derive(Default)]
    struct MemoryDatabaseProvider {
        provisioned: RefCell<Vec<InstallationId>>,
    }

    impl AppDatabaseProvider for MemoryDatabaseProvider {
        type Access = ();
        type Error = TestError;

        fn provision_installation(
            &self,
            bindings: &[AppDatabaseBinding],
        ) -> Result<(), Self::Error> {
            let installation_id = *bindings
                .first()
                .ok_or(TestError("empty database bindings"))?
                .owner()
                .installation_id();
            if !self.provisioned.borrow().contains(&installation_id) {
                self.provisioned.borrow_mut().push(installation_id);
            }
            Ok(())
        }

        fn installation_state(
            &self,
            installation_id: &InstallationId,
        ) -> Result<AppDatabaseInstallationState, Self::Error> {
            Ok(if self.provisioned.borrow().contains(installation_id) {
                AppDatabaseInstallationState::Active
            } else {
                AppDatabaseInstallationState::Absent
            })
        }

        fn access(&self, _binding: &AppDatabaseBinding) -> Result<Self::Access, Self::Error> {
            Ok(())
        }

        fn retain_installation(
            &self,
            installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            let before = self.provisioned.borrow().len();
            self.provisioned
                .borrow_mut()
                .retain(|stored| stored != installation_id);
            Ok(before != self.provisioned.borrow().len())
        }

        fn restore_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }
    }

    #[derive(Default)]
    struct MemoryOidcRepository {
        clients: RefCell<Vec<OidcClientRecord>>,
    }

    impl OidcClientRepository for MemoryOidcRepository {
        type Error = TestError;

        fn insert(&self, client: &OidcClientRecord) -> Result<(), Self::Error> {
            self.clients.borrow_mut().push(client.clone());
            Ok(())
        }

        fn find_active_by_id(
            &self,
            client_id: &OidcClientId,
        ) -> Result<Option<OidcClientRecord>, Self::Error> {
            Ok(self
                .clients
                .borrow()
                .iter()
                .find(|client| client.client_id() == client_id && client.is_active())
                .cloned())
        }

        fn find_active_by_installation(
            &self,
            installation_id: &InstallationId,
        ) -> Result<Option<OidcClientRecord>, Self::Error> {
            Ok(self
                .clients
                .borrow()
                .iter()
                .find(|client| client.installation_id() == installation_id && client.is_active())
                .cloned())
        }

        fn revoke_for_installation(
            &self,
            _installation_id: &InstallationId,
            _revoked_at: UnixTimestamp,
        ) -> Result<usize, Self::Error> {
            Ok(0)
        }
    }

    struct MemoryOriginResolver;

    impl InstalledAppOriginResolver for MemoryOriginResolver {
        type Error = TestError;

        fn resolve_origin(
            &self,
            _app: &InstalledApp,
            _entrypoint: &RuntimeEntrypointId,
        ) -> Result<String, Self::Error> {
            Ok("https://apps.rumahl.test".to_owned())
        }
    }

    #[derive(Default)]
    struct MemorySnapshotRepository {
        snapshot: RefCell<Option<PlatformSnapshot>>,
    }

    impl PlatformSnapshotRepository for MemorySnapshotRepository {
        type Error = TestError;

        fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error> {
            Ok(self.snapshot.borrow().clone())
        }

        fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error> {
            self.snapshot.replace(Some(snapshot.clone()));
            Ok(())
        }
    }

    struct StoredSecret {
        id: SecretId,
        owner: rumahl_core::AppIdentity,
        purpose: SecretPurpose,
        value: Vec<u8>,
        created_at: UnixTimestamp,
    }

    #[derive(Default)]
    struct MemorySecretStore {
        secrets: RefCell<Vec<StoredSecret>>,
    }

    impl SecretStore for MemorySecretStore {
        type Error = TestError;

        fn insert(&self, secret: &SecretRecord) -> Result<(), Self::Error> {
            if self.secrets.borrow().iter().any(|stored| {
                stored.owner.installation_id() == secret.owner().installation_id()
                    && stored.purpose == *secret.purpose()
            }) {
                return Err(TestError("duplicate secret"));
            }
            self.secrets.borrow_mut().push(StoredSecret {
                id: *secret.id(),
                owner: secret.owner().clone(),
                purpose: secret.purpose().clone(),
                value: secret.value().as_bytes().to_vec(),
                created_at: secret.created_at(),
            });
            Ok(())
        }

        fn find_by_owner_and_purpose(
            &self,
            owner: &rumahl_core::AppIdentity,
            purpose: &SecretPurpose,
        ) -> Result<Option<SecretRecord>, Self::Error> {
            Ok(self
                .secrets
                .borrow()
                .iter()
                .find(|stored| &stored.owner == owner && &stored.purpose == purpose)
                .map(|stored| {
                    SecretRecord::restore(
                        stored.id,
                        stored.owner.clone(),
                        stored.purpose.clone(),
                        SecretValue::new(stored.value.clone()).unwrap(),
                        stored.created_at,
                    )
                }))
        }

        fn remove_for_installation(
            &self,
            installation_id: &InstallationId,
        ) -> Result<usize, Self::Error> {
            let before = self.secrets.borrow().len();
            self.secrets
                .borrow_mut()
                .retain(|stored| stored.owner.installation_id() != installation_id);
            Ok(before - self.secrets.borrow().len())
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    struct DeliveryAttempt {
        operation_id: AppOperationId,
        installation_id: InstallationId,
        client_id: String,
        secret_digest: OidcClientSecretDigest,
    }

    #[derive(Default)]
    struct MemoryRuntimeSecrets {
        fail_next: Cell<bool>,
        attempts: RefCell<Vec<DeliveryAttempt>>,
    }

    impl RuntimeSecretDelivery for MemoryRuntimeSecrets {
        type Error = TestError;

        fn deliver_oidc_client_secret(
            &self,
            operation_id: &AppOperationId,
            app: &InstalledApp,
            client_id: &OidcClientId,
            client_secret: &OidcClientSecret,
        ) -> Result<(), Self::Error> {
            self.attempts.borrow_mut().push(DeliveryAttempt {
                operation_id: *operation_id,
                installation_id: *app.installation_id(),
                client_id: client_id.to_string(),
                secret_digest: client_secret.digest(),
            });
            if self.fail_next.replace(false) {
                return Err(TestError("injected delivery failure"));
            }
            Ok(())
        }

        fn remove_for_installation(
            &self,
            _operation_id: &AppOperationId,
            _installation_id: &InstallationId,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    type TestRunner = AppOperationRunner<
        MemoryJournal,
        MemoryDatabaseProvider,
        MemoryOidcRepository,
        MemoryOriginResolver,
        MemorySnapshotRepository,
        MemorySecretStore,
        MemoryRuntimeSecrets,
    >;

    fn runner() -> TestRunner {
        AppOperationRunner::new(
            MemoryJournal::default(),
            MemoryDatabaseProvider::default(),
            MemoryOidcRepository::default(),
            MemoryOriginResolver,
            MemorySnapshotRepository::default(),
            MemorySecretStore::default(),
            MemoryRuntimeSecrets::default(),
        )
    }

    fn container_manifest() -> AppManifest {
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
            AppId::parse("com.rumahl.cloud").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Cloud",
            runtime,
        )
        .unwrap();
        manifest
            .add_database(AppDatabaseDeclaration::new(
                AppDatabaseId::parse("primary").unwrap(),
            ))
            .unwrap();
        manifest
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Confidential,
                    RuntimeEntrypointId::parse("main").unwrap(),
                    OidcCallbackPath::parse("/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId, OidcScope::Profile],
                )
                .unwrap(),
            )
            .unwrap();
        manifest
    }

    #[test]
    fn installs_all_resources_and_commits_before_publishing_live_state() {
        let runner = runner();
        let mut state = PlatformState::new();
        let grants = InMemoryGrantStore::new();

        let result = runner
            .install(container_manifest(), &mut state, &grants)
            .unwrap();

        assert_eq!(state.installed_apps().len(), 1);
        assert_eq!(runner.database_provider().provisioned.borrow().len(), 1);
        assert_eq!(runner.oidc_repository().clients.borrow().len(), 1);
        assert_eq!(runner.secret_store().secrets.borrow().len(), 1);
        assert_eq!(runner.runtime_secrets().attempts.borrow().len(), 1);
        assert_eq!(
            runner
                .journal()
                .find(result.operation_id())
                .unwrap()
                .unwrap()
                .phase(),
            AppOperationPhase::Committed
        );
        assert_eq!(
            runner
                .snapshot_repository()
                .load()
                .unwrap()
                .unwrap()
                .installed_apps()
                .len(),
            1
        );
    }

    #[test]
    fn resumes_after_secret_delivery_failure_with_same_client_and_secret() {
        let runner = runner();
        runner.runtime_secrets().fail_next.set(true);
        let mut state = PlatformState::new();
        let grants = InMemoryGrantStore::new();

        let error = runner
            .install(container_manifest(), &mut state, &grants)
            .unwrap_err();

        assert!(matches!(
            error,
            AppOperationRunnerError::RuntimeSecretDelivery(_)
        ));
        assert!(state.installed_apps().is_empty());
        let incomplete = runner.journal().list_incomplete().unwrap();
        assert_eq!(incomplete.len(), 1);
        let oidc_step = incomplete[0]
            .steps()
            .iter()
            .find(|step: &&AppOperationStep| step.resource() == AppOperationResource::OidcClient)
            .unwrap();
        assert_eq!(oidc_step.state(), AppOperationResourceState::Applying);

        let report = runner.recover_incomplete(&mut state, &grants).unwrap();

        assert_eq!(report.resumed(), 1);
        assert_eq!(report.committed(), 1);
        assert_eq!(state.installed_apps().len(), 1);
        let attempts = runner.runtime_secrets().attempts.borrow();
        assert_eq!(attempts.len(), 2);
        assert!(attempts[0] == attempts[1]);
        assert_eq!(runner.oidc_repository().clients.borrow().len(), 1);
        assert_eq!(runner.secret_store().secrets.borrow().len(), 1);
        assert!(runner.journal().list_incomplete().unwrap().is_empty());
    }
}
