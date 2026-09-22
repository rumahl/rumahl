use std::error::Error;
use std::fmt;

use rumahl_core::{
    AppLifecycle, AppLifecycleError, AppManifest, AppUninstallResult, InMemoryGrantStore,
    InstallationId, InstalledApp, PlatformState, UnixTimestamp,
};

use crate::{
    InstalledAppOriginResolver, OidcClientRegistrar, OidcClientRegistration,
    OidcClientRegistrationError, OidcClientRepository,
};

/// Coordinates the core app lifecycle with its per-installation OIDC client.
///
/// The live platform state and grant store are replaced only after the OIDC
/// repository operation succeeds. Repository implementations are required to
/// make each individual insert and revocation atomic.
#[derive(Debug)]
pub struct OidcAppLifecycle<R, O> {
    app_lifecycle: AppLifecycle,
    client_registrar: OidcClientRegistrar<R, O>,
}

pub struct OidcAppInstallResult {
    app: InstalledApp,
    oidc_registration: Option<OidcClientRegistration>,
}

#[derive(Debug)]
pub struct OidcAppUninstallResult {
    app_uninstall: AppUninstallResult,
    revoked_oidc_clients: usize,
}

#[derive(Debug)]
pub enum OidcAppInstallError<RepositoryError, ResolverError> {
    AppLifecycle(AppLifecycleError),
    OidcRegistration(OidcClientRegistrationError<RepositoryError, ResolverError>),
}

#[derive(Debug)]
pub enum OidcAppUninstallError<RepositoryError> {
    AppLifecycle(AppLifecycleError),
    OidcRevocation(RepositoryError),
}

impl<R, O> OidcAppLifecycle<R, O>
where
    R: OidcClientRepository,
    O: InstalledAppOriginResolver,
{
    pub fn new(repository: R, origin_resolver: O) -> Self {
        Self {
            app_lifecycle: AppLifecycle::new(),
            client_registrar: OidcClientRegistrar::new(repository, origin_resolver),
        }
    }

    pub fn install(
        &self,
        manifest: AppManifest,
        state: &mut PlatformState,
        created_at: UnixTimestamp,
    ) -> Result<OidcAppInstallResult, OidcAppInstallError<R::Error, O::Error>> {
        let mut staged = state.clone();
        let app = self
            .app_lifecycle
            .install(manifest, &mut staged)
            .map_err(OidcAppInstallError::AppLifecycle)?;
        let oidc_registration = self
            .client_registrar
            .register_installed_app(&staged, app.installation_id(), created_at)
            .map_err(OidcAppInstallError::OidcRegistration)?;

        *state = staged;

        Ok(OidcAppInstallResult {
            app,
            oidc_registration,
        })
    }

    pub fn uninstall(
        &self,
        installation_id: &InstallationId,
        state: &mut PlatformState,
        grant_store: &mut InMemoryGrantStore,
        revoked_at: UnixTimestamp,
    ) -> Result<OidcAppUninstallResult, OidcAppUninstallError<R::Error>> {
        let mut staged_state = state.clone();
        let mut staged_grants = grant_store.clone();
        let app_uninstall = self
            .app_lifecycle
            .uninstall(installation_id, &mut staged_state, &mut staged_grants)
            .map_err(OidcAppUninstallError::AppLifecycle)?;
        let revoked_oidc_clients = self
            .client_registrar
            .repository()
            .revoke_for_installation(installation_id, revoked_at)
            .map_err(OidcAppUninstallError::OidcRevocation)?;

        *state = staged_state;
        *grant_store = staged_grants;

        Ok(OidcAppUninstallResult {
            app_uninstall,
            revoked_oidc_clients,
        })
    }

    pub fn client_repository(&self) -> &R {
        self.client_registrar.repository()
    }
}

impl OidcAppInstallResult {
    pub fn app(&self) -> &InstalledApp {
        &self.app
    }

    pub fn oidc_registration(&self) -> Option<&OidcClientRegistration> {
        self.oidc_registration.as_ref()
    }

    pub fn into_parts(self) -> (InstalledApp, Option<OidcClientRegistration>) {
        (self.app, self.oidc_registration)
    }
}

impl fmt::Debug for OidcAppInstallResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcAppInstallResult")
            .field("app", &self.app)
            .field("has_oidc_registration", &self.oidc_registration.is_some())
            .finish()
    }
}

impl OidcAppUninstallResult {
    pub fn app_uninstall(&self) -> &AppUninstallResult {
        &self.app_uninstall
    }

    pub fn revoked_oidc_clients(&self) -> usize {
        self.revoked_oidc_clients
    }
}

impl<RepositoryError, ResolverError> fmt::Display
    for OidcAppInstallError<RepositoryError, ResolverError>
where
    RepositoryError: Error,
    ResolverError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppLifecycle(error) => write!(f, "app lifecycle failed: {error}"),
            Self::OidcRegistration(error) => {
                write!(f, "app OIDC registration failed: {error}")
            }
        }
    }
}

impl<RepositoryError, ResolverError> Error for OidcAppInstallError<RepositoryError, ResolverError>
where
    RepositoryError: Error + 'static,
    ResolverError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AppLifecycle(error) => Some(error),
            Self::OidcRegistration(error) => Some(error),
        }
    }
}

impl<RepositoryError> fmt::Display for OidcAppUninstallError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppLifecycle(error) => write!(f, "app lifecycle failed: {error}"),
            Self::OidcRevocation(_) => write!(f, "app OIDC revocation failed"),
        }
    }
}

impl<RepositoryError> Error for OidcAppUninstallError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AppLifecycle(error) => Some(error),
            Self::OidcRevocation(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use rumahl_core::{
        AppDatabaseDeclaration, AppDatabaseId, AppId, AppVersion, GrantAuthority,
        GrantIssuerPolicy, OidcCallbackPath, OidcClientDeclaration, OidcClientType, OidcScope,
        PackagePath, PermissionId, PermissionScope, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace, ResourceRef, RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
        UserId, UserIdentity, UserRole,
    };

    use super::*;
    use crate::{OidcClientId, OidcClientRecord};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestError(&'static str);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    #[derive(Debug, Default)]
    struct MemoryRepository {
        clients: RefCell<Vec<OidcClientRecord>>,
        fail_insert: Cell<bool>,
        fail_revoke: Cell<bool>,
    }

    impl OidcClientRepository for MemoryRepository {
        type Error = TestError;

        fn insert(&self, client: &OidcClientRecord) -> Result<(), Self::Error> {
            if self.fail_insert.get() {
                return Err(TestError("insert failed"));
            }

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
                .find(|client| client.client_id() == client_id)
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
                .find(|client| client.installation_id() == installation_id)
                .cloned())
        }

        fn revoke_for_installation(
            &self,
            installation_id: &InstallationId,
            _revoked_at: UnixTimestamp,
        ) -> Result<usize, Self::Error> {
            if self.fail_revoke.get() {
                return Err(TestError("revocation failed"));
            }

            let before = self.clients.borrow().len();
            self.clients
                .borrow_mut()
                .retain(|client| client.installation_id() != installation_id);
            Ok(before - self.clients.borrow().len())
        }
    }

    #[derive(Debug)]
    struct FixedOriginResolver;

    impl InstalledAppOriginResolver for FixedOriginResolver {
        type Error = TestError;

        fn resolve_origin(
            &self,
            _app: &InstalledApp,
            _entrypoint: &RuntimeEntrypointId,
        ) -> Result<String, Self::Error> {
            Ok("https://notes.rumahl.local/".to_owned())
        }
    }

    fn manifest(with_oidc: bool) -> AppManifest {
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

        if with_oidc {
            manifest
                .declare_oidc_client(
                    OidcClientDeclaration::new(
                        OidcClientType::Public,
                        RuntimeEntrypointId::parse("main").unwrap(),
                        OidcCallbackPath::parse("/oidc/callback").unwrap(),
                        vec![OidcScope::OpenId, OidcScope::Profile],
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        manifest
    }

    fn app_grant(app: &InstalledApp) -> rumahl_core::PermissionGrant {
        let user = UserIdentity::new(UserId::new());
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(*user.id(), UserRole::User);

        GrantAuthority::new()
            .issue(
                &policy,
                user.into(),
                app.identity().clone().into(),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![ResourceRef::new(
                    ResourceNamespace::parse("rumahl.files").unwrap(),
                    ResourceKind::parse("file").unwrap(),
                    ResourceKey::parse("document-1").unwrap(),
                )],
            )
            .unwrap()
    }

    #[test]
    fn installs_app_and_oidc_client_as_one_live_state_change() {
        let lifecycle = OidcAppLifecycle::new(MemoryRepository::default(), FixedOriginResolver);
        let mut state = PlatformState::new();

        let result = lifecycle
            .install(manifest(true), &mut state, UnixTimestamp::from_seconds(100))
            .unwrap();
        let registration = result.oidc_registration().unwrap();

        assert_eq!(state.installed_apps().len(), 1);
        assert_eq!(
            registration.client().installation_id(),
            result.app().installation_id()
        );
        assert!(registration.client_secret().is_none());
        assert!(
            lifecycle
                .client_repository()
                .find_active_by_installation(result.app().installation_id())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn oidc_insert_failure_does_not_publish_staged_app() {
        let repository = MemoryRepository::default();
        repository.fail_insert.set(true);
        let lifecycle = OidcAppLifecycle::new(repository, FixedOriginResolver);
        let mut state = PlatformState::new();

        assert!(matches!(
            lifecycle.install(manifest(true), &mut state, UnixTimestamp::from_seconds(100),),
            Err(OidcAppInstallError::OidcRegistration(
                OidcClientRegistrationError::Repository(TestError("insert failed"))
            ))
        ));
        assert!(state.installed_apps().is_empty());
        assert!(lifecycle.client_repository().clients.borrow().is_empty());
    }

    #[test]
    fn app_without_oidc_does_not_touch_client_repository() {
        let repository = MemoryRepository::default();
        repository.fail_insert.set(true);
        let lifecycle = OidcAppLifecycle::new(repository, FixedOriginResolver);
        let mut state = PlatformState::new();
        let mut manifest = manifest(false);
        manifest
            .add_database(AppDatabaseDeclaration::new(
                AppDatabaseId::parse("primary").unwrap(),
            ))
            .unwrap();

        let result = lifecycle
            .install(manifest, &mut state, UnixTimestamp::from_seconds(100))
            .unwrap();

        assert!(result.oidc_registration().is_none());
        assert_eq!(state.installed_apps().len(), 1);
        assert_eq!(state.database_registry().len(), 1);
        assert!(lifecycle.client_repository().clients.borrow().is_empty());
    }

    #[test]
    fn uninstall_revokes_client_and_commits_staged_core_state() {
        let lifecycle = OidcAppLifecycle::new(MemoryRepository::default(), FixedOriginResolver);
        let mut state = PlatformState::new();
        let installed = lifecycle
            .install(manifest(true), &mut state, UnixTimestamp::from_seconds(100))
            .unwrap();
        let installation_id = *installed.app().installation_id();
        let mut grants = InMemoryGrantStore::new();
        grants.insert(app_grant(installed.app()));

        let result = lifecycle
            .uninstall(
                &installation_id,
                &mut state,
                &mut grants,
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        assert_eq!(result.revoked_oidc_clients(), 1);
        assert_eq!(
            result.app_uninstall().app().installation_id(),
            &installation_id
        );
        assert!(state.installed_apps().is_empty());
        assert!(grants.is_empty());
        assert!(
            lifecycle
                .client_repository()
                .find_active_by_installation(&installation_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn oidc_revocation_failure_keeps_app_and_grants_live() {
        let lifecycle = OidcAppLifecycle::new(MemoryRepository::default(), FixedOriginResolver);
        let mut state = PlatformState::new();
        let installed = lifecycle
            .install(manifest(true), &mut state, UnixTimestamp::from_seconds(100))
            .unwrap();
        let installation_id = *installed.app().installation_id();
        let mut grants = InMemoryGrantStore::new();
        grants.insert(app_grant(installed.app()));
        lifecycle.client_repository().fail_revoke.set(true);

        assert!(matches!(
            lifecycle.uninstall(
                &installation_id,
                &mut state,
                &mut grants,
                UnixTimestamp::from_seconds(200),
            ),
            Err(OidcAppUninstallError::OidcRevocation(TestError(
                "revocation failed"
            )))
        ));
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&installation_id)
                .is_some()
        );
        assert_eq!(grants.len(), 1);
        assert!(
            lifecycle
                .client_repository()
                .find_active_by_installation(&installation_id)
                .unwrap()
                .is_some()
        );
    }
}
