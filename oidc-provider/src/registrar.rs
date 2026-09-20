use std::error::Error;
use std::fmt;

use rumahl_core::{
    InstallationId, InstalledApp, OidcClientDeclaration, OidcClientType, PlatformState,
    RuntimeEntrypointId, SecretPurpose, SecretRecord, SecretStore, SecretValue, SecretValueError,
    UnixTimestamp,
};

use crate::{
    OidcClientId, OidcClientIdGenerationError, OidcClientRecord, OidcClientRecordError,
    OidcClientRepository, OidcClientSecret, OidcClientSecretGenerationError,
    OidcClientSecretParseError, OidcRedirectUri, OidcRedirectUriError,
};

pub const OIDC_CLIENT_SECRET_PURPOSE: &str = "rumahl.oidc.client-secret";

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

#[derive(Debug)]
pub enum OidcClientProvisioningError<RepositoryError, ResolverError, SecretStoreError> {
    InstallationNotFound,
    OriginResolution(ResolverError),
    InvalidRedirectUri(OidcRedirectUriError),
    ClientIdGeneration(OidcClientIdGenerationError),
    ClientSecretGeneration(OidcClientSecretGenerationError),
    InvalidClient(OidcClientRecordError),
    Repository(RepositoryError),
    SecretStore(SecretStoreError),
    InvalidStoredSecretEncoding(std::str::Utf8Error),
    InvalidStoredSecret(OidcClientSecretParseError),
    InvalidSecretValue(SecretValueError),
    ExistingClientMismatch,
    MissingStoredSecret,
    UnexpectedStoredSecret,
}

pub type OidcClientProvisioningResult<RepositoryError, ResolverError, SecretStoreError> = Result<
    Option<OidcClientRegistration>,
    OidcClientProvisioningError<RepositoryError, ResolverError, SecretStoreError>,
>;

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

    /// Registers a declared client or reconstructs an interrupted registration.
    ///
    /// Confidential client material is encrypted in `secret_store` before its
    /// digest is inserted into the client repository. A replay returns the same
    /// active client and secret after verifying all declaration-derived fields.
    pub fn register_or_recover_installed_app<S>(
        &self,
        state: &PlatformState,
        installation_id: &InstallationId,
        created_at: UnixTimestamp,
        secret_store: &S,
    ) -> OidcClientProvisioningResult<R::Error, O::Error, S::Error>
    where
        S: SecretStore,
    {
        let app = state
            .installed_apps()
            .get_by_installation_id(installation_id)
            .ok_or(OidcClientProvisioningError::InstallationNotFound)?;
        let Some(declaration) = app.manifest().oidc_client() else {
            return Ok(None);
        };
        let origin = self
            .origin_resolver
            .resolve_origin(app, declaration.callback_entrypoint())
            .map_err(OidcClientProvisioningError::OriginResolution)?;
        let redirect_uri =
            OidcRedirectUri::from_platform_origin(&origin, declaration.callback_path())
                .map_err(OidcClientProvisioningError::InvalidRedirectUri)?;
        let purpose = oidc_client_secret_purpose();
        let stored_secret = secret_store
            .find_by_owner_and_purpose(app.identity(), &purpose)
            .map_err(OidcClientProvisioningError::SecretStore)?;
        let existing_client = self
            .repository
            .find_active_by_installation(installation_id)
            .map_err(OidcClientProvisioningError::Repository)?;

        match declaration.client_type() {
            OidcClientType::Public => {
                if stored_secret.is_some() {
                    return Err(OidcClientProvisioningError::UnexpectedStoredSecret);
                }

                if let Some(client) = existing_client {
                    if !client_matches_declaration(&client, app, declaration, &redirect_uri, None) {
                        return Err(OidcClientProvisioningError::ExistingClientMismatch);
                    }

                    return Ok(Some(OidcClientRegistration {
                        client,
                        client_secret: None,
                    }));
                }

                let client_id = OidcClientId::generate()
                    .map_err(OidcClientProvisioningError::ClientIdGeneration)?;
                let client =
                    build_client(client_id, app, declaration, redirect_uri, None, created_at)
                        .map_err(OidcClientProvisioningError::InvalidClient)?;
                self.repository
                    .insert(&client)
                    .map_err(OidcClientProvisioningError::Repository)?;

                Ok(Some(OidcClientRegistration {
                    client,
                    client_secret: None,
                }))
            }
            OidcClientType::Confidential => {
                if let Some(client) = existing_client {
                    let stored_secret =
                        stored_secret.ok_or(OidcClientProvisioningError::MissingStoredSecret)?;
                    let client_secret = decode_stored_client_secret(stored_secret)?;

                    if !client_matches_declaration(
                        &client,
                        app,
                        declaration,
                        &redirect_uri,
                        Some(&client_secret),
                    ) {
                        return Err(OidcClientProvisioningError::ExistingClientMismatch);
                    }

                    return Ok(Some(OidcClientRegistration {
                        client,
                        client_secret: Some(client_secret),
                    }));
                }

                let client_id = OidcClientId::generate()
                    .map_err(OidcClientProvisioningError::ClientIdGeneration)?;
                let client_secret = match stored_secret {
                    Some(stored_secret) => decode_stored_client_secret(stored_secret)?,
                    None => {
                        let client_secret = OidcClientSecret::generate()
                            .map_err(OidcClientProvisioningError::ClientSecretGeneration)?;
                        let encoded = client_secret.encode();
                        let value = SecretValue::new(encoded.as_bytes().to_vec())
                            .map_err(OidcClientProvisioningError::InvalidSecretValue)?;
                        let secret =
                            SecretRecord::new(app.identity().clone(), purpose, value, created_at);
                        secret_store
                            .insert(&secret)
                            .map_err(OidcClientProvisioningError::SecretStore)?;
                        client_secret
                    }
                };
                let client = build_client(
                    client_id,
                    app,
                    declaration,
                    redirect_uri,
                    Some(&client_secret),
                    created_at,
                )
                .map_err(OidcClientProvisioningError::InvalidClient)?;
                self.repository
                    .insert(&client)
                    .map_err(OidcClientProvisioningError::Repository)?;

                Ok(Some(OidcClientRegistration {
                    client,
                    client_secret: Some(client_secret),
                }))
            }
        }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

fn oidc_client_secret_purpose() -> SecretPurpose {
    SecretPurpose::parse(OIDC_CLIENT_SECRET_PURPOSE)
        .expect("OIDC client secret purpose is a valid built-in identifier")
}

fn decode_stored_client_secret<RepositoryError, ResolverError, SecretStoreError>(
    stored_secret: SecretRecord,
) -> Result<
    OidcClientSecret,
    OidcClientProvisioningError<RepositoryError, ResolverError, SecretStoreError>,
> {
    let encoded = std::str::from_utf8(stored_secret.value().as_bytes())
        .map_err(OidcClientProvisioningError::InvalidStoredSecretEncoding)?;

    OidcClientSecret::parse(encoded).map_err(OidcClientProvisioningError::InvalidStoredSecret)
}

fn build_client(
    client_id: OidcClientId,
    app: &InstalledApp,
    declaration: &OidcClientDeclaration,
    redirect_uri: OidcRedirectUri,
    client_secret: Option<&OidcClientSecret>,
    created_at: UnixTimestamp,
) -> Result<OidcClientRecord, OidcClientRecordError> {
    OidcClientRecord::restore(
        client_id,
        *app.installation_id(),
        app.identity().app_id().clone(),
        app.manifest().display_name(),
        declaration.client_type(),
        redirect_uri,
        declaration.scopes().to_vec(),
        client_secret.map(OidcClientSecret::digest),
        created_at,
        None,
    )
}

fn client_matches_declaration(
    client: &OidcClientRecord,
    app: &InstalledApp,
    declaration: &OidcClientDeclaration,
    redirect_uri: &OidcRedirectUri,
    client_secret: Option<&OidcClientSecret>,
) -> bool {
    client.is_active()
        && client.installation_id() == app.installation_id()
        && client.app_id() == app.identity().app_id()
        && client.display_name() == app.manifest().display_name()
        && client.client_type() == declaration.client_type()
        && client.redirect_uri() == redirect_uri
        && client.scopes() == declaration.scopes()
        && match (client.client_secret_digest(), client_secret) {
            (None, None) => true,
            (Some(digest), Some(secret)) => digest.verifies(secret),
            _ => false,
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

impl<RepositoryError, ResolverError, SecretStoreError> fmt::Display
    for OidcClientProvisioningError<RepositoryError, ResolverError, SecretStoreError>
where
    RepositoryError: Error,
    ResolverError: Error,
    SecretStoreError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallationNotFound => write!(f, "installed app was not found"),
            Self::OriginResolution(_) => write!(f, "installed app origin could not be resolved"),
            Self::InvalidRedirectUri(error) => write!(f, "OIDC redirect URI is invalid: {error}"),
            Self::ClientIdGeneration(_) => write!(f, "OIDC client ID could not be generated"),
            Self::ClientSecretGeneration(_) => {
                write!(f, "OIDC client secret could not be generated")
            }
            Self::InvalidClient(error) => write!(f, "OIDC client is invalid: {error}"),
            Self::Repository(_) => write!(f, "OIDC client could not be persisted"),
            Self::SecretStore(_) => write!(f, "OIDC client secret store failed"),
            Self::InvalidStoredSecretEncoding(_) | Self::InvalidStoredSecret(_) => {
                write!(f, "stored OIDC client secret is invalid")
            }
            Self::InvalidSecretValue(error) => {
                write!(f, "OIDC client secret cannot be stored: {error}")
            }
            Self::ExistingClientMismatch => {
                write!(
                    f,
                    "existing OIDC client does not match the installed app declaration"
                )
            }
            Self::MissingStoredSecret => {
                write!(f, "confidential OIDC client secret is missing")
            }
            Self::UnexpectedStoredSecret => {
                write!(
                    f,
                    "public OIDC client unexpectedly has stored secret material"
                )
            }
        }
    }
}

impl<RepositoryError, ResolverError, SecretStoreError> Error
    for OidcClientProvisioningError<RepositoryError, ResolverError, SecretStoreError>
where
    RepositoryError: Error + 'static,
    ResolverError: Error + 'static,
    SecretStoreError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OriginResolution(error) => Some(error),
            Self::InvalidRedirectUri(error) => Some(error),
            Self::ClientIdGeneration(error) => Some(error),
            Self::ClientSecretGeneration(error) => Some(error),
            Self::InvalidClient(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::SecretStore(error) => Some(error),
            Self::InvalidStoredSecretEncoding(error) => Some(error),
            Self::InvalidStoredSecret(error) => Some(error),
            Self::InvalidSecretValue(error) => Some(error),
            Self::InstallationNotFound
            | Self::ExistingClientMismatch
            | Self::MissingStoredSecret
            | Self::UnexpectedStoredSecret => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use rumahl_core::{
        AppId, AppLifecycle, AppManifest, AppVersion, OidcCallbackPath, OidcClientDeclaration,
        OidcScope, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
        RuntimeEntrypoint, SecretId,
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

    #[derive(Clone)]
    struct MemorySecret {
        id: SecretId,
        owner: rumahl_core::AppIdentity,
        purpose: SecretPurpose,
        value: Vec<u8>,
        created_at: UnixTimestamp,
    }

    #[derive(Default)]
    struct MemorySecretStore {
        secrets: RefCell<Vec<MemorySecret>>,
        inserts: Cell<usize>,
    }

    impl SecretStore for MemorySecretStore {
        type Error = TestError;

        fn insert(&self, secret: &SecretRecord) -> Result<(), Self::Error> {
            if self.secrets.borrow().iter().any(|stored| {
                stored.owner.installation_id() == secret.owner().installation_id()
                    && stored.purpose == *secret.purpose()
            }) {
                return Err(TestError);
            }

            self.secrets.borrow_mut().push(MemorySecret {
                id: *secret.id(),
                owner: secret.owner().clone(),
                purpose: secret.purpose().clone(),
                value: secret.value().as_bytes().to_vec(),
                created_at: secret.created_at(),
            });
            self.inserts.set(self.inserts.get() + 1);
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
                .retain(|secret| secret.owner.installation_id() != installation_id);
            Ok(before - self.secrets.borrow().len())
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
    fn confidential_registration_recovers_same_client_and_encrypted_secret() {
        let (state, installation_id) = installed_confidential_container_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://cloud.rumahl.local/"),
        );
        let secret_store = MemorySecretStore::default();

        let first = registrar
            .register_or_recover_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(100),
                &secret_store,
            )
            .unwrap()
            .unwrap();
        let first_client_id = first.client().client_id().clone();
        let first_secret = first.client_secret().unwrap().encode();
        let recovered = registrar
            .register_or_recover_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(101),
                &secret_store,
            )
            .unwrap()
            .unwrap();

        assert_eq!(recovered.client().client_id(), &first_client_id);
        assert_eq!(recovered.client_secret().unwrap().encode(), first_secret);
        assert_eq!(registrar.repository().clients.borrow().len(), 1);
        assert_eq!(secret_store.inserts.get(), 1);
    }

    #[test]
    fn confidential_registration_uses_secret_saved_before_client_insert() {
        let (state, installation_id) = installed_confidential_container_client();
        let app = state
            .installed_apps()
            .get_by_installation_id(&installation_id)
            .unwrap();
        let secret_store = MemorySecretStore::default();
        let preexisting_secret = OidcClientSecret::generate().unwrap();
        let encoded = preexisting_secret.encode();
        secret_store
            .insert(&SecretRecord::new(
                app.identity().clone(),
                oidc_client_secret_purpose(),
                SecretValue::new(encoded.as_bytes().to_vec()).unwrap(),
                UnixTimestamp::from_seconds(99),
            ))
            .unwrap();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://cloud.rumahl.local/"),
        );

        let registration = registrar
            .register_or_recover_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(100),
                &secret_store,
            )
            .unwrap()
            .unwrap();

        assert_eq!(registration.client_secret().unwrap().encode(), encoded);
        assert!(registration.client().verifies_client_secret(&encoded));
        assert_eq!(secret_store.inserts.get(), 1);
    }

    #[test]
    fn public_recovery_never_creates_secret_material() {
        let (state, installation_id) = installed_public_web_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://notes.rumahl.local/"),
        );
        let secret_store = MemorySecretStore::default();

        for timestamp in [100, 101] {
            let registration = registrar
                .register_or_recover_installed_app(
                    &state,
                    &installation_id,
                    UnixTimestamp::from_seconds(timestamp),
                    &secret_store,
                )
                .unwrap()
                .unwrap();
            assert!(registration.client_secret().is_none());
        }

        assert_eq!(registrar.repository().clients.borrow().len(), 1);
        assert_eq!(secret_store.inserts.get(), 0);
        assert!(secret_store.secrets.borrow().is_empty());
    }

    #[test]
    fn existing_confidential_client_without_stored_secret_fails_closed() {
        let (state, installation_id) = installed_confidential_container_client();
        let registrar = OidcClientRegistrar::new(
            MemoryRepository::default(),
            FixedOriginResolver("https://cloud.rumahl.local/"),
        );
        registrar
            .register_installed_app(&state, &installation_id, UnixTimestamp::from_seconds(100))
            .unwrap();

        assert!(matches!(
            registrar.register_or_recover_installed_app(
                &state,
                &installation_id,
                UnixTimestamp::from_seconds(101),
                &MemorySecretStore::default(),
            ),
            Err(OidcClientProvisioningError::MissingStoredSecret)
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
