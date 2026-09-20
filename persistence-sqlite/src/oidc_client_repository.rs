use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{AppId, InstallationId, OidcClientType, OidcScope, UnixTimestamp};
use rumahl_oidc_provider::{
    OidcClientId, OidcClientRecord, OidcClientRepository, OidcClientSecretDigest, OidcRedirectUri,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::oidc_schema;

#[derive(Debug)]
pub struct SqliteOidcClientRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteOidcClientRepositoryError {
    Database(rusqlite::Error),
    InvalidClientId(String),
    InvalidInstallationId(String),
    InvalidAppId(String),
    InvalidClientType(String),
    InvalidRedirectUri(String),
    InvalidClientSecretDigestLength(usize),
    InvalidScope(String),
    InvalidTimestamp { field: &'static str, value: i64 },
    TimestampOutsideSqliteRange { field: &'static str, value: u64 },
    InvalidClient(String),
    LockPoisoned,
}

impl SqliteOidcClientRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteOidcClientRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteOidcClientRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteOidcClientRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        oidc_schema::initialize(&connection).map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteOidcClientRepositoryError {
        SqliteOidcClientRepositoryError::Database(error)
    }

    fn find(
        &self,
        column: &'static str,
        value: &str,
    ) -> Result<Option<OidcClientRecord>, SqliteOidcClientRepositoryError> {
        debug_assert!(matches!(column, "client_id" | "installation_id"));
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteOidcClientRepositoryError::LockPoisoned)?;
        let sql = format!(
            "SELECT client_id, installation_id, app_id, display_name, client_type,
                    redirect_uri, client_secret_digest, created_at, revoked_at
             FROM oidc_client
             WHERE {column} = ?1 AND revoked_at IS NULL"
        );
        let stored = connection
            .query_row(&sql, [value], |row| {
                Ok(StoredClient {
                    client_id: row.get(0)?,
                    installation_id: row.get(1)?,
                    app_id: row.get(2)?,
                    display_name: row.get(3)?,
                    client_type: row.get(4)?,
                    redirect_uri: row.get(5)?,
                    client_secret_digest: row.get(6)?,
                    created_at: row.get(7)?,
                    revoked_at: row.get(8)?,
                })
            })
            .optional()
            .map_err(Self::database_error)?;

        stored
            .map(|stored| {
                let scopes = load_scopes(&connection, &stored.client_id)?;
                stored.into_domain(scopes)
            })
            .transpose()
    }
}

impl OidcClientRepository for SqliteOidcClientRepository {
    type Error = SqliteOidcClientRepositoryError;

    fn insert(&self, client: &OidcClientRecord) -> Result<(), Self::Error> {
        let created_at = write_timestamp("created_at", client.created_at())?;
        let revoked_at = client
            .revoked_at()
            .map(|value| write_timestamp("revoked_at", value))
            .transpose()?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteOidcClientRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;

        transaction
            .execute(
                "INSERT INTO oidc_client (
                     client_id, installation_id, app_id, display_name, client_type,
                     redirect_uri, client_secret_digest, created_at, revoked_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    client.client_id().as_str(),
                    client.installation_id().to_string(),
                    client.app_id().as_str(),
                    client.display_name(),
                    client_type_name(client.client_type()),
                    client.redirect_uri().as_str(),
                    client
                        .client_secret_digest()
                        .map(|digest| &digest.as_bytes()[..]),
                    created_at,
                    revoked_at,
                ],
            )
            .map_err(Self::database_error)?;

        for scope in client.scopes() {
            transaction
                .execute(
                    "INSERT INTO oidc_client_scope (client_id, scope) VALUES (?1, ?2)",
                    params![client.client_id().as_str(), scope.as_str()],
                )
                .map_err(Self::database_error)?;
        }

        transaction.commit().map_err(Self::database_error)
    }

    fn find_active_by_id(
        &self,
        client_id: &OidcClientId,
    ) -> Result<Option<OidcClientRecord>, Self::Error> {
        self.find("client_id", client_id.as_str())
    }

    fn find_active_by_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<Option<OidcClientRecord>, Self::Error> {
        self.find("installation_id", &installation_id.to_string())
    }

    fn revoke_for_installation(
        &self,
        installation_id: &InstallationId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, Self::Error> {
        let revoked_at = write_timestamp("revoked_at", revoked_at)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteOidcClientRepositoryError::LockPoisoned)?;

        connection
            .execute(
                "UPDATE oidc_client
                 SET revoked_at = ?2
                 WHERE installation_id = ?1 AND revoked_at IS NULL",
                params![installation_id.to_string(), revoked_at],
            )
            .map_err(Self::database_error)
    }
}

struct StoredClient {
    client_id: String,
    installation_id: String,
    app_id: String,
    display_name: String,
    client_type: String,
    redirect_uri: String,
    client_secret_digest: Option<Vec<u8>>,
    created_at: i64,
    revoked_at: Option<i64>,
}

impl StoredClient {
    fn into_domain(
        self,
        scopes: Vec<OidcScope>,
    ) -> Result<OidcClientRecord, SqliteOidcClientRepositoryError> {
        let client_secret_digest = self
            .client_secret_digest
            .map(|bytes| {
                let length = bytes.len();
                let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                    SqliteOidcClientRepositoryError::InvalidClientSecretDigestLength(length)
                })?;
                Ok(OidcClientSecretDigest::from_bytes(bytes))
            })
            .transpose()?;

        OidcClientRecord::restore(
            OidcClientId::parse(&self.client_id).map_err(|_| {
                SqliteOidcClientRepositoryError::InvalidClientId(self.client_id.clone())
            })?,
            InstallationId::parse(&self.installation_id).map_err(|_| {
                SqliteOidcClientRepositoryError::InvalidInstallationId(self.installation_id.clone())
            })?,
            AppId::parse(&self.app_id)
                .map_err(|_| SqliteOidcClientRepositoryError::InvalidAppId(self.app_id.clone()))?,
            self.display_name,
            parse_client_type(&self.client_type)?,
            OidcRedirectUri::parse(&self.redirect_uri).map_err(|_| {
                SqliteOidcClientRepositoryError::InvalidRedirectUri(self.redirect_uri.clone())
            })?,
            scopes,
            client_secret_digest,
            read_timestamp("created_at", self.created_at)?,
            self.revoked_at
                .map(|value| read_timestamp("revoked_at", value))
                .transpose()?,
        )
        .map_err(|error| SqliteOidcClientRepositoryError::InvalidClient(error.to_string()))
    }
}

fn load_scopes(
    connection: &Connection,
    client_id: &str,
) -> Result<Vec<OidcScope>, SqliteOidcClientRepositoryError> {
    let mut statement = connection
        .prepare("SELECT scope FROM oidc_client_scope WHERE client_id = ?1 ORDER BY rowid")
        .map_err(SqliteOidcClientRepository::database_error)?;
    let scopes = statement
        .query_map([client_id], |row| row.get::<_, String>(0))
        .map_err(SqliteOidcClientRepository::database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteOidcClientRepository::database_error)?;

    scopes
        .into_iter()
        .map(|scope| {
            OidcScope::parse(&scope).ok_or(SqliteOidcClientRepositoryError::InvalidScope(scope))
        })
        .collect()
}

fn client_type_name(client_type: OidcClientType) -> &'static str {
    match client_type {
        OidcClientType::Public => "public",
        OidcClientType::Confidential => "confidential",
    }
}

fn parse_client_type(client_type: &str) -> Result<OidcClientType, SqliteOidcClientRepositoryError> {
    match client_type {
        "public" => Ok(OidcClientType::Public),
        "confidential" => Ok(OidcClientType::Confidential),
        value => Err(SqliteOidcClientRepositoryError::InvalidClientType(
            value.to_owned(),
        )),
    }
}

fn read_timestamp(
    field: &'static str,
    value: i64,
) -> Result<UnixTimestamp, SqliteOidcClientRepositoryError> {
    let value = u64::try_from(value)
        .map_err(|_| SqliteOidcClientRepositoryError::InvalidTimestamp { field, value })?;

    Ok(UnixTimestamp::from_seconds(value))
}

fn write_timestamp(
    field: &'static str,
    value: UnixTimestamp,
) -> Result<i64, SqliteOidcClientRepositoryError> {
    i64::try_from(value.as_seconds()).map_err(|_| {
        SqliteOidcClientRepositoryError::TimestampOutsideSqliteRange {
            field,
            value: value.as_seconds(),
        }
    })
}

impl fmt::Display for SqliteOidcClientRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite OIDC client repository failed: {error}"),
            Self::InvalidClientId(_) => write!(f, "stored OIDC client ID is invalid"),
            Self::InvalidInstallationId(_) => {
                write!(f, "stored OIDC installation ID is invalid")
            }
            Self::InvalidAppId(_) => write!(f, "stored OIDC app ID is invalid"),
            Self::InvalidClientType(value) => {
                write!(f, "stored OIDC client type '{value}' is invalid")
            }
            Self::InvalidRedirectUri(_) => write!(f, "stored OIDC redirect URI is invalid"),
            Self::InvalidClientSecretDigestLength(length) => write!(
                f,
                "stored OIDC client secret digest length {length} is invalid"
            ),
            Self::InvalidScope(scope) => write!(f, "stored OIDC scope '{scope}' is invalid"),
            Self::InvalidTimestamp { field, value } => {
                write!(f, "stored OIDC {field} timestamp {value} is invalid")
            }
            Self::TimestampOutsideSqliteRange { field, value } => write!(
                f,
                "OIDC {field} timestamp {value} is outside SQLite integer range"
            ),
            Self::InvalidClient(message) => write!(f, "stored OIDC client is invalid: {message}"),
            Self::LockPoisoned => write!(f, "SQLite OIDC client repository lock is poisoned"),
        }
    }
}

impl Error for SqliteOidcClientRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidClientId(_)
            | Self::InvalidInstallationId(_)
            | Self::InvalidAppId(_)
            | Self::InvalidClientType(_)
            | Self::InvalidRedirectUri(_)
            | Self::InvalidClientSecretDigestLength(_)
            | Self::InvalidScope(_)
            | Self::InvalidTimestamp { .. }
            | Self::TimestampOutsideSqliteRange { .. }
            | Self::InvalidClient(_)
            | Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_core::{
        AppManifest, AppVersion, InMemoryGrantStore, OidcCallbackPath, OidcClientDeclaration,
        OidcScope, PackagePath, PlatformState, PublisherId, RuntimeDescriptor, RuntimeEntrypoint,
        RuntimeEntrypointId,
    };
    use rumahl_oidc_provider::{
        InstalledAppOriginResolver, OidcAppLifecycle, OidcClientSecret, OidcClientSecretDigest,
    };

    use super::*;

    fn database_path(test_name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("rumahl-{test_name}-{nonce}.sqlite3"))
    }

    fn remove_database(path: &Path) {
        for suffix in ["", "-shm", "-wal"] {
            let candidate = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
            let _ = fs::remove_file(candidate);
        }
    }

    fn confidential_client(
        installation_id: InstallationId,
        secret_digest: OidcClientSecretDigest,
    ) -> OidcClientRecord {
        OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            installation_id,
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Confidential,
            OidcRedirectUri::parse("https://cloud.rumahl.local/apps/oidc/callback").unwrap(),
            vec![OidcScope::OpenId, OidcScope::Profile],
            Some(secret_digest),
            UnixTimestamp::from_seconds(100),
            None,
        )
        .unwrap()
    }

    #[derive(Debug)]
    struct FixedOriginResolver;

    impl InstalledAppOriginResolver for FixedOriginResolver {
        type Error = Infallible;

        fn resolve_origin(
            &self,
            _app: &rumahl_core::InstalledApp,
            _entrypoint: &RuntimeEntrypointId,
        ) -> Result<String, Self::Error> {
            Ok("https://notes.rumahl.local/".to_owned())
        }
    }

    fn public_web_manifest() -> AppManifest {
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

        manifest
    }

    #[test]
    fn confidential_client_survives_reopen_and_revocation_without_storing_clear_secret() {
        let path = database_path("oidc-client");
        let installation_id = InstallationId::new();
        let secret = OidcClientSecret::generate().unwrap();
        let encoded_secret = secret.encode();
        let client = confidential_client(installation_id, secret.digest());
        let client_id = client.client_id().clone();

        {
            let repository = SqliteOidcClientRepository::open(&path).unwrap();
            repository.insert(&client).unwrap();
            repository
                .connection
                .lock()
                .unwrap()
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .unwrap();
        }

        let database = fs::read(&path).unwrap();
        assert!(
            !database
                .windows(encoded_secret.len())
                .any(|window| window == encoded_secret.as_bytes())
        );

        let repository = SqliteOidcClientRepository::open(&path).unwrap();
        let recovered = repository.find_active_by_id(&client_id).unwrap().unwrap();

        assert_eq!(recovered.installation_id(), &installation_id);
        assert_eq!(recovered.scopes(), &[OidcScope::OpenId, OidcScope::Profile]);
        assert!(recovered.verifies_client_secret(&encoded_secret));
        assert!(recovered.matches_redirect_uri("https://cloud.rumahl.local/apps/oidc/callback"));
        assert!(!recovered.matches_redirect_uri("https://cloud.rumahl.local/apps/oidc/callback/"));
        assert_eq!(
            repository
                .revoke_for_installation(&installation_id, UnixTimestamp::from_seconds(200),)
                .unwrap(),
            1
        );
        assert!(repository.find_active_by_id(&client_id).unwrap().is_none());

        drop(repository);
        remove_database(&path);
    }

    #[test]
    fn one_installation_cannot_receive_two_client_registrations() {
        let repository = SqliteOidcClientRepository::open_in_memory().unwrap();
        let installation_id = InstallationId::new();
        let first_secret = OidcClientSecret::generate().unwrap();
        let second_secret = OidcClientSecret::generate().unwrap();

        repository
            .insert(&confidential_client(installation_id, first_secret.digest()))
            .unwrap();
        let error = repository
            .insert(&confidential_client(
                installation_id,
                second_secret.digest(),
            ))
            .unwrap_err();

        assert!(matches!(
            error,
            SqliteOidcClientRepositoryError::Database(_)
        ));
    }

    #[test]
    fn sqlite_registry_participates_in_oidc_aware_app_lifecycle() {
        let lifecycle = OidcAppLifecycle::new(
            SqliteOidcClientRepository::open_in_memory().unwrap(),
            FixedOriginResolver,
        );
        let mut state = PlatformState::new();
        let installed = lifecycle
            .install(
                public_web_manifest(),
                &mut state,
                UnixTimestamp::from_seconds(100),
            )
            .unwrap();
        let installation_id = *installed.app().installation_id();

        assert!(
            lifecycle
                .client_repository()
                .find_active_by_installation(&installation_id)
                .unwrap()
                .is_some()
        );

        let result = lifecycle
            .uninstall(
                &installation_id,
                &mut state,
                &mut InMemoryGrantStore::new(),
                UnixTimestamp::from_seconds(200),
            )
            .unwrap();

        assert_eq!(result.revoked_oidc_clients(), 1);
        assert!(state.installed_apps().is_empty());
        assert!(
            lifecycle
                .client_repository()
                .find_active_by_installation(&installation_id)
                .unwrap()
                .is_none()
        );
    }
}
