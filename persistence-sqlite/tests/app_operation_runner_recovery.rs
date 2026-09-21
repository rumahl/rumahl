use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rumahl_app_operations::{
    AppOperationRunner, AppOperationRunnerError, AppRuntimeServices, RuntimeSecretDelivery,
};
use rumahl_core::{
    AppDatabaseDeclaration, AppDatabaseId, AppDatabaseInstallationState, AppDatabaseProvider,
    AppId, AppManifest, AppOperationPhase, AppOperationRepository, AppRuntimeInstallationState,
    AppRuntimeProvider, AppVersion, InMemoryGrantStore, InstalledApp, OidcCallbackPath,
    OidcClientDeclaration, OidcClientType, OidcScope, PackagePath, PlatformSnapshotRepository,
    PlatformState, PublisherId, RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint,
    RuntimeEntrypointId, SecretPurpose, SecretStore,
};
use rumahl_oidc_provider::{
    InstalledAppOriginResolver, OIDC_CLIENT_SECRET_PURPOSE, OidcClientId, OidcClientRepository,
    OidcClientSecret, OidcClientSecretDigest,
};
use rumahl_persistence_sqlite::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
    SqliteAppDatabaseProvider, SqliteAppOperationRepository, SqliteOidcClientRepository,
    SqliteSecretStore, SqliteSnapshotRepository,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestError(&'static str);

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Error for TestError {}

#[derive(Clone)]
struct TestKeyProvider;

impl SecretEncryptionKeyProvider for TestKeyProvider {
    type Error = Infallible;

    fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
        Ok(test_key())
    }

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
        Ok((id.as_str() == "test-key-1").then(test_key))
    }
}

fn test_key() -> SecretEncryptionKey {
    SecretEncryptionKey::new(
        SecretEncryptionKeyId::parse("test-key-1").unwrap(),
        [0x5a; 32],
    )
}

struct TestOriginResolver;

impl InstalledAppOriginResolver for TestOriginResolver {
    type Error = TestError;

    fn resolve_origin(
        &self,
        _app: &InstalledApp,
        _entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error> {
        Ok("https://apps.rumahl.test".to_owned())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct DeliveryAttempt {
    operation_id: String,
    installation_id: String,
    client_id: String,
    secret_digest: OidcClientSecretDigest,
}

#[derive(Default)]
struct DeliveryState {
    fail_next: bool,
    fail_next_removal: bool,
    attempts: Vec<DeliveryAttempt>,
    removals: Vec<(String, String)>,
}

#[derive(Clone)]
struct TestRuntimeSecrets {
    state: Arc<Mutex<DeliveryState>>,
}

#[derive(Clone)]
struct TestRuntimeProvider {
    state: Arc<Mutex<Vec<(InstalledApp, AppRuntimeInstallationState)>>>,
}

impl AppRuntimeProvider for TestRuntimeProvider {
    type Error = TestError;

    fn prepare_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        let mut runtimes = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        if let Some((existing, _)) = runtimes
            .iter()
            .find(|(existing, _)| existing.installation_id() == app.installation_id())
        {
            return if existing == app {
                Ok(())
            } else {
                Err(TestError("conflicting runtime"))
            };
        }
        runtimes.push((app.clone(), AppRuntimeInstallationState::Prepared));
        Ok(())
    }

    fn activate_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
        let mut runtimes = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        let (existing, state) = runtimes
            .iter_mut()
            .find(|(existing, _)| existing.installation_id() == app.installation_id())
            .ok_or(TestError("runtime is not prepared"))?;
        if existing != app {
            return Err(TestError("conflicting runtime"));
        }
        *state = AppRuntimeInstallationState::Active;
        Ok(())
    }

    fn installation_state(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<AppRuntimeInstallationState, Self::Error> {
        Ok(self
            .state
            .lock()
            .map_err(|_| TestError("lock poisoned"))?
            .iter()
            .find(|(app, _)| app.installation_id() == installation_id)
            .map(|(_, state)| *state)
            .unwrap_or(AppRuntimeInstallationState::Absent))
    }

    fn deactivate_installation(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<bool, Self::Error> {
        let mut runtimes = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        let Some((_, state)) = runtimes
            .iter_mut()
            .find(|(app, _)| app.installation_id() == installation_id)
        else {
            return Ok(false);
        };
        if *state == AppRuntimeInstallationState::Active {
            *state = AppRuntimeInstallationState::Prepared;
            return Ok(true);
        }
        Ok(false)
    }

    fn remove_installation(
        &self,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<bool, Self::Error> {
        if self.installation_state(installation_id)? == AppRuntimeInstallationState::Active {
            return Err(TestError("runtime is active"));
        }
        let mut runtimes = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        let before = runtimes.len();
        runtimes.retain(|(app, _)| app.installation_id() != installation_id);
        Ok(before != runtimes.len())
    }
}

impl RuntimeSecretDelivery for TestRuntimeSecrets {
    type Error = TestError;

    fn deliver_oidc_client_secret(
        &self,
        operation_id: &rumahl_core::AppOperationId,
        app: &InstalledApp,
        client_id: &OidcClientId,
        client_secret: &OidcClientSecret,
    ) -> Result<(), Self::Error> {
        let mut state = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        state.attempts.push(DeliveryAttempt {
            operation_id: operation_id.to_string(),
            installation_id: app.installation_id().to_string(),
            client_id: client_id.to_string(),
            secret_digest: client_secret.digest(),
        });
        if state.fail_next {
            state.fail_next = false;
            return Err(TestError("injected delivery failure"));
        }
        Ok(())
    }

    fn remove_for_installation(
        &self,
        operation_id: &rumahl_core::AppOperationId,
        installation_id: &rumahl_core::InstallationId,
    ) -> Result<(), Self::Error> {
        let mut state = self.state.lock().map_err(|_| TestError("lock poisoned"))?;
        state
            .removals
            .push((operation_id.to_string(), installation_id.to_string()));
        if state.fail_next_removal {
            state.fail_next_removal = false;
            return Err(TestError("injected removal failure"));
        }
        Ok(())
    }
}

fn test_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rumahl-operation-runner-{nonce}"))
}

fn manifest() -> AppManifest {
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

fn runner(
    state_path: &Path,
    database_root: &Path,
    runtime_provider: TestRuntimeProvider,
    delivery: TestRuntimeSecrets,
) -> AppOperationRunner<
    SqliteAppOperationRepository,
    SqliteAppDatabaseProvider,
    TestRuntimeProvider,
    SqliteOidcClientRepository,
    TestOriginResolver,
    SqliteSnapshotRepository,
    SqliteSecretStore<TestKeyProvider>,
    TestRuntimeSecrets,
> {
    AppOperationRunner::new(
        SqliteAppOperationRepository::open(state_path).unwrap(),
        SqliteAppDatabaseProvider::open(database_root).unwrap(),
        AppRuntimeServices::new(runtime_provider, delivery),
        SqliteOidcClientRepository::open(state_path).unwrap(),
        TestOriginResolver,
        SqliteSnapshotRepository::open(state_path).unwrap(),
        SqliteSecretStore::open(state_path, TestKeyProvider).unwrap(),
    )
}

#[test]
fn resumes_installation_across_reopened_sqlite_repositories() {
    let root = test_root();
    fs::create_dir(&root).unwrap();
    let state_path = root.join("platform.sqlite3");
    let database_root = root.join("app-databases");
    let delivery_state = Arc::new(Mutex::new(DeliveryState {
        fail_next: true,
        fail_next_removal: false,
        attempts: Vec::new(),
        removals: Vec::new(),
    }));
    let delivery = TestRuntimeSecrets {
        state: Arc::clone(&delivery_state),
    };
    let runtime_provider = TestRuntimeProvider {
        state: Arc::new(Mutex::new(Vec::new())),
    };
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();

    {
        let runner = runner(
            &state_path,
            &database_root,
            runtime_provider.clone(),
            delivery.clone(),
        );
        let error = runner.install(manifest(), &mut state, &grants).unwrap_err();
        assert!(matches!(
            error,
            AppOperationRunnerError::RuntimeSecretDelivery(_)
        ));
        assert!(state.installed_apps().is_empty());
        assert_eq!(runner.journal().list_incomplete().unwrap().len(), 1);
    }

    {
        let runner = runner(&state_path, &database_root, runtime_provider, delivery);
        let report = runner.recover_incomplete(&mut state, &mut grants).unwrap();
        assert_eq!(report.resumed(), 1);
        assert_eq!(report.committed(), 1);
        assert_eq!(report.compensated(), 0);
        assert!(runner.journal().list_incomplete().unwrap().is_empty());
        assert_eq!(state.installed_apps().len(), 1);
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

        let app = &state.installed_apps().apps()[0];
        let client = runner
            .oidc_repository()
            .find_active_by_installation(app.installation_id())
            .unwrap()
            .unwrap();
        let purpose = SecretPurpose::parse(OIDC_CLIENT_SECRET_PURPOSE).unwrap();
        assert!(
            runner
                .secret_store()
                .find_by_owner_and_purpose(app.identity(), &purpose)
                .unwrap()
                .is_some()
        );

        let attempts = delivery_state.lock().unwrap();
        assert_eq!(attempts.attempts.len(), 2);
        assert!(attempts.attempts[0] == attempts.attempts[1]);
        assert_eq!(attempts.attempts[1].client_id, client.client_id().as_str());
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resumes_uninstall_across_reopened_sqlite_repositories() {
    let root = test_root();
    fs::create_dir(&root).unwrap();
    let state_path = root.join("platform.sqlite3");
    let database_root = root.join("app-databases");
    let delivery_state = Arc::new(Mutex::new(DeliveryState::default()));
    let delivery = TestRuntimeSecrets {
        state: Arc::clone(&delivery_state),
    };
    let runtime_provider = TestRuntimeProvider {
        state: Arc::new(Mutex::new(Vec::new())),
    };
    let mut state = PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    let installed_app;
    let uninstall_operation_id;

    {
        let runner = runner(
            &state_path,
            &database_root,
            runtime_provider.clone(),
            delivery.clone(),
        );
        installed_app = runner
            .install(manifest(), &mut state, &grants)
            .unwrap()
            .app()
            .clone();
        delivery_state.lock().unwrap().fail_next_removal = true;

        let error = runner
            .uninstall(installed_app.installation_id(), &mut state, &mut grants)
            .unwrap_err();

        assert!(matches!(
            error,
            AppOperationRunnerError::RuntimeSecretDelivery(_)
        ));
        assert_eq!(state.installed_apps().len(), 1);
        let incomplete = runner.journal().list_incomplete().unwrap();
        assert_eq!(incomplete.len(), 1);
        uninstall_operation_id = *incomplete[0].id();
        assert_eq!(
            runner
                .database_provider()
                .installation_state(installed_app.installation_id())
                .unwrap(),
            AppDatabaseInstallationState::Active
        );
        assert_eq!(
            runner
                .runtime_provider()
                .installation_state(installed_app.installation_id())
                .unwrap(),
            AppRuntimeInstallationState::Prepared
        );
    }

    {
        let runner = runner(&state_path, &database_root, runtime_provider, delivery);
        let report = runner.recover_incomplete(&mut state, &mut grants).unwrap();

        assert_eq!(report.resumed(), 1);
        assert_eq!(report.committed(), 1);
        assert_eq!(report.compensated(), 0);
        assert!(state.installed_apps().is_empty());
        assert!(
            runner
                .snapshot_repository()
                .load()
                .unwrap()
                .unwrap()
                .installed_apps()
                .is_empty()
        );
        assert_eq!(
            runner
                .database_provider()
                .installation_state(installed_app.installation_id())
                .unwrap(),
            AppDatabaseInstallationState::Retained
        );
        assert_eq!(
            runner
                .runtime_provider()
                .installation_state(installed_app.installation_id())
                .unwrap(),
            AppRuntimeInstallationState::Absent
        );
        assert!(
            runner
                .oidc_repository()
                .find_active_by_installation(installed_app.installation_id())
                .unwrap()
                .is_none()
        );
        let purpose = SecretPurpose::parse(OIDC_CLIENT_SECRET_PURPOSE).unwrap();
        assert!(
            runner
                .secret_store()
                .find_by_owner_and_purpose(installed_app.identity(), &purpose)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runner
                .journal()
                .find(&uninstall_operation_id)
                .unwrap()
                .unwrap()
                .phase(),
            AppOperationPhase::Committed
        );
        assert!(runner.journal().list_incomplete().unwrap().is_empty());
        assert_eq!(delivery_state.lock().unwrap().removals.len(), 2);
    }

    fs::remove_dir_all(root).unwrap();
}
