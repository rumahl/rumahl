use std::error::Error;
use std::fmt;

use rumahl_core::{
    InstallationId, InstalledApp, OidcClientType, PlatformState, RuntimeEntrypointId, UnixTimestamp,
};

use crate::{
    OidcClientId, OidcClientIdGenerationError, OidcClientRecord, OidcClientRecordError,
    OidcClientRepository, OidcClientSecret, OidcClientSecretGenerationError, OidcRedirectUri,
    OidcRedirectUriError,
};

pub trait InstalledAppOriginResolver {
    type Error: Error + Send + Sync + 'static;

    fn resolve_origin(
        &self,
        app: &InstalledApp,
        entrypoint: &RuntimeEntrypointId,
    ) -> Result<String, Self::Error>;
}

#[derive(Debug)]
pub struct OidcClientRegistrar<R, O> {
    repository: R,
    origin_resolver: O,
}

pub struct OidcClientRegistration {
    client: OidcClientRecord,
    client_secret: Option<OidcClientSecret>,
}

#[derive(Debug)]
pub enum OidcClientRegistrationError<RepositoryError, ResolverError> {
    InstallationNotFound,
    ClientAlreadyRegistered,
    OriginResolution(ResolverError),
    InvalidRedirectUri(OidcRedirectUriError),
    ClientIdGeneration(OidcClientIdGenerationError),
    ClientSecretGeneration(OidcClientSecretGenerationError),
    InvalidClient(OidcClientRecordError),
    Repository(RepositoryError),
}

impl<R, O> OidcClientRegistrar<R, O>
where
    R: OidcClientRepository,
    O: InstalledAppOriginResolver,
{
    pub fn new(repository: R, origin_resolver: O) -> Self {
        Self {
            repository,
            origin_resolver,
        }
    }

    pub fn register_installed_app(
        &self,
        state: &PlatformState,
        installation_id: &InstallationId,
        created_at: UnixTimestamp,
    ) -> Result<Option<OidcClientRegistration>, OidcClientRegistrationError<R::Error, O::Error>>
    {
        let app = state
            .installed_apps()
            .get_by_installation_id(installation_id)
            .ok_or(OidcClientRegistrationError::InstallationNotFound)?;
        let Some(declaration) = app.manifest().oidc_client() else {
            return Ok(None);
        };

        if self
            .repository
            .find_active_by_installation(installation_id)
            .map_err(OidcClientRegistrationError::Repository)?
            .is_some()
        {
            return Err(OidcClientRegistrationError::ClientAlreadyRegistered);
        }

        let origin = self
            .origin_resolver
            .resolve_origin(app, declaration.callback_entrypoint())
            .map_err(OidcClientRegistrationError::OriginResolution)?;
        let redirect_uri =
            OidcRedirectUri::from_platform_origin(&origin, declaration.callback_path())
                .map_err(OidcClientRegistrationError::InvalidRedirectUri)?;
        let client_id =
            OidcClientId::generate().map_err(OidcClientRegistrationError::ClientIdGeneration)?;
        let client_secret = match declaration.client_type() {
            OidcClientType::Public => None,
            OidcClientType::Confidential => Some(
                OidcClientSecret::generate()
                    .map_err(OidcClientRegistrationError::ClientSecretGeneration)?,
            ),
        };
        let client = OidcClientRecord::restore(
            client_id,
            *installation_id,
            app.identity().app_id().clone(),
            app.manifest().display_name(),
            declaration.client_type(),
            redirect_uri,
            declaration.scopes().to_vec(),
            client_secret.as_ref().map(OidcClientSecret::digest),
            created_at,
            None,
        )
        .map_err(OidcClientRegistrationError::InvalidClient)?;

        self.repository
            .insert(&client)
            .map_err(OidcClientRegistrationError::Repository)?;

        Ok(Some(OidcClientRegistration {
            client,
            client_secret,
        }))
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl OidcClientRegistration {
    pub fn client(&self) -> &OidcClientRecord {
        &self.client
    }

    pub fn client_secret(&self) -> Option<&OidcClientSecret> {
        self.client_secret.as_ref()
    }

    pub fn into_client_secret(self) -> Option<OidcClientSecret> {
        self.client_secret
    }
}

impl fmt::Debug for OidcClientRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcClientRegistration")
            .field("client", &self.client)
            .field("has_client_secret", &self.client_secret.is_some())
            .finish()
    }
}

impl<RepositoryError, ResolverError> fmt::Display
    for OidcClientRegistrationError<RepositoryError, ResolverError>
where
    RepositoryError: Error,
    ResolverError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallationNotFound => write!(f, "installed app was not found"),
            Self::ClientAlreadyRegistered => {
                write!(f, "installed app already has an active OIDC client")
            }
            Self::OriginResolution(_) => write!(f, "installed app origin could not be resolved"),
            Self::InvalidRedirectUri(error) => write!(f, "OIDC redirect URI is invalid: {error}"),
            Self::ClientIdGeneration(_) => write!(f, "OIDC client ID could not be generated"),
            Self::ClientSecretGeneration(_) => {
                write!(f, "OIDC client secret could not be generated")
            }
            Self::InvalidClient(error) => write!(f, "OIDC client is invalid: {error}"),
            Self::Repository(_) => write!(f, "OIDC client could not be persisted"),
        }
    }
}

impl<RepositoryError, ResolverError> Error
    for OidcClientRegistrationError<RepositoryError, ResolverError>
where
    RepositoryError: Error + 'static,
    ResolverError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OriginResolution(error) => Some(error),
            Self::InvalidRedirectUri(error) => Some(error),
            Self::ClientIdGeneration(error) => Some(error),
            Self::ClientSecretGeneration(error) => Some(error),
            Self::InvalidClient(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::InstallationNotFound | Self::ClientAlreadyRegistered => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use rumahl_core::{
        AppId, AppLifecycle, AppManifest, AppVersion, OidcCallbackPath, OidcClientDeclaration,
        OidcScope, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
        RuntimeEntrypoint,
    };

    use super::*;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test error")
        }
    }

    impl Error for TestError {}

    #[derive(Debug, Default)]
    struct MemoryRepository {
        clients: RefCell<Vec<OidcClientRecord>>,
    }

    impl OidcClientRepository for MemoryRepository {
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

    #[derive(Debug)]
    struct FixedOriginResolver(&'static str);

    impl InstalledAppOriginResolver for FixedOriginResolver {
        type Error = TestError;

        fn resolve_origin(
            &self,
            _app: &InstalledApp,
            _entrypoint: &RuntimeEntrypointId,
        ) -> Result<String, Self::Error> {
            Ok(self.0.to_owned())
        }
    }

    fn installed_public_web_client() -> (PlatformState, InstallationId) {
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
        let mut state = PlatformState::new();
        let installed = AppLifecycle::new().install(manifest, &mut state).unwrap();

        (state, *installed.installation_id())
    }

    fn installed_confidential_container_client() -> (PlatformState, InstallationId) {
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
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Confidential,
                    RuntimeEntrypointId::parse("main").unwrap(),
                    OidcCallbackPath::parse("/apps/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId, OidcScope::Profile],
                )
                .unwrap(),
            )
            .unwrap();
        let mut state = PlatformState::new();
        let installed = AppLifecycle::new().install(manifest, &mut state).unwrap();

        (state, *installed.installation_id())
    }

    #[test]
    fn registers_public_web_client_without_secret_and_with_exact_redirect() {
        let (state, installation_id) = installed_public_web_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://notes.rumahl.local/"),
        );

        let registration = registrar
            .register_installed_app(&state, &installation_id, UnixTimestamp::from_seconds(100))
            .unwrap()
            .unwrap();

        assert!(registration.client_secret().is_none());
        assert_eq!(
            registration.client().token_endpoint_auth_method(),
            crate::OidcTokenEndpointAuthMethod::None
        );
        assert_eq!(
            registration.client().code_challenge_method(),
            crate::OidcCodeChallengeMethod::S256
        );
        assert!(
            registration
                .client()
                .matches_redirect_uri("https://notes.rumahl.local/oidc/callback")
        );
        assert!(
            !registration
                .client()
                .matches_redirect_uri("https://notes.rumahl.local/oidc/callback/")
        );
    }

    #[test]
    fn confidential_client_secret_is_returned_once_and_only_its_digest_is_registered() {
        let (state, installation_id) = installed_confidential_container_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://cloud.rumahl.local/"),
        );

        let registration = registrar
            .register_installed_app(&state, &installation_id, UnixTimestamp::from_seconds(100))
            .unwrap()
            .unwrap();
        let encoded_secret = registration.client_secret().unwrap().encode();

        assert!(
            registration
                .client()
                .verifies_client_secret(&encoded_secret)
        );
        assert!(registration.client().client_secret_digest().is_some());
        assert!(!format!("{registration:?}").contains(encoded_secret.as_str()));
        assert!(matches!(
            registrar.register_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(101),
            ),
            Err(OidcClientRegistrationError::ClientAlreadyRegistered)
        ));
    }

    #[test]
    fn rejects_non_https_origin_even_for_valid_installed_app() {
        let (state, installation_id) = installed_public_web_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("http://notes.rumahl.local/"),
        );

        assert!(matches!(
            registrar.register_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(100),
            ),
            Err(OidcClientRegistrationError::InvalidRedirectUri(
                OidcRedirectUriError::HttpsRequired
            ))
        ));
        assert!(registrar.repository().clients.borrow().is_empty());
    }
}
