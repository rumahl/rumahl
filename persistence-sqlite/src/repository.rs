use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{PlatformSnapshot, PlatformSnapshotRepository};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::wire::WireSnapshot;

const SNAPSHOT_ID: i64 = 1;

#[derive(Debug)]
pub struct SqliteSnapshotRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteSnapshotRepositoryError {
    Database(rusqlite::Error),
    Json(serde_json::Error),
    InvalidSnapshot(String),
    InvalidStoredVersion(i64),
    VersionMismatch { column: u32, payload: u32 },
    LockPoisoned,
}

impl SqliteSnapshotRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteSnapshotRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteSnapshotRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteSnapshotRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS platform_snapshot (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     snapshot_version INTEGER NOT NULL CHECK (snapshot_version >= 0),
                     payload TEXT NOT NULL
                 );",
            )
            .map_err(Self::database_error)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn database_error(error: rusqlite::Error) -> SqliteSnapshotRepositoryError {
        SqliteSnapshotRepositoryError::Database(error)
    }
}

impl PlatformSnapshotRepository for SqliteSnapshotRepository {
    type Error = SqliteSnapshotRepositoryError;

    fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqliteSnapshotRepositoryError::LockPoisoned)?;
        let stored = connection
            .query_row(
                "SELECT snapshot_version, payload
                 FROM platform_snapshot
                 WHERE id = ?1",
                [SNAPSHOT_ID],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(Self::database_error)?;

        let Some((stored_version, payload)) = stored else {
            return Ok(None);
        };

        let column_version = u32::try_from(stored_version)
            .map_err(|_| SqliteSnapshotRepositoryError::InvalidStoredVersion(stored_version))?;
        let wire: WireSnapshot =
            serde_json::from_str(&payload).map_err(SqliteSnapshotRepositoryError::Json)?;

        if column_version != wire.version() {
            return Err(SqliteSnapshotRepositoryError::VersionMismatch {
                column: column_version,
                payload: wire.version(),
            });
        }

        wire.into_domain()
            .map(Some)
            .map_err(|error| SqliteSnapshotRepositoryError::InvalidSnapshot(error.to_string()))
    }

    fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error> {
        let payload = serde_json::to_string(&WireSnapshot::capture(snapshot))
            .map_err(SqliteSnapshotRepositoryError::Json)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteSnapshotRepositoryError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;

        transaction
            .execute(
                "INSERT INTO platform_snapshot (id, snapshot_version, payload)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                     snapshot_version = excluded.snapshot_version,
                     payload = excluded.payload",
                params![SNAPSHOT_ID, i64::from(snapshot.version()), payload],
            )
            .map_err(Self::database_error)?;
        transaction.commit().map_err(Self::database_error)
    }
}

impl fmt::Display for SqliteSnapshotRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite snapshot repository failed: {error}"),
            Self::Json(error) => write!(f, "snapshot JSON is invalid: {error}"),
            Self::InvalidSnapshot(message) => write!(f, "stored snapshot is invalid: {message}"),
            Self::InvalidStoredVersion(version) => {
                write!(
                    f,
                    "stored snapshot version {version} is outside the supported integer range"
                )
            }
            Self::VersionMismatch { column, payload } => write!(
                f,
                "stored snapshot version mismatch: column has {column}, payload has {payload}"
            ),
            Self::LockPoisoned => write!(f, "SQLite snapshot repository lock is poisoned"),
        }
    }
}

impl Error for SqliteSnapshotRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidSnapshot(_)
            | Self::InvalidStoredVersion(_)
            | Self::VersionMismatch { .. }
            | Self::LockPoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use rumahl_core::{
        AppDatabaseDeclaration, AppDatabaseId, AppId, AppLifecycle, AppManifest, AppVersion,
        CapabilityId, CommandAction, CommandContributionDeclaration, ContributionId, EventName,
        GrantAuthority, GrantIssuerPolicy, InMemoryGrantStore, OidcCallbackPath,
        OidcClientDeclaration, OidcClientType, OidcScope, PLATFORM_SNAPSHOT_VERSION, PackagePath,
        PermissionId, PermissionRequest, PermissionScope, PlatformRecovery, PlatformState,
        PublisherId, ResourceKey, ResourceKind, ResourceNamespace, ResourceRef, RuntimeDescriptor,
        RuntimeEntrypoint, RuntimeEntrypointId, SearchContributionDeclaration, UserId,
        UserIdentity, UserRole,
    };

    fn populated_snapshot() -> PlatformSnapshot {
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
            AppVersion::new(1, 2, 3),
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
            .add_database(AppDatabaseDeclaration::new(
                AppDatabaseId::parse("primary").unwrap(),
            ))
            .unwrap();
        manifest
            .add_permission_request(PermissionRequest::new(
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::UserSelected,
                true,
                Some("Open selected files".to_owned()),
            ))
            .unwrap();
        manifest
            .add_provided_capability(CapabilityId::parse("rumahl.search.query").unwrap())
            .unwrap();
        manifest
            .add_contribution(
                CommandContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.open").unwrap(),
                    "Open Notes",
                    CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
                )
                .unwrap()
                .into(),
            )
            .unwrap();
        manifest
            .add_contribution(
                SearchContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.search").unwrap(),
                    CapabilityId::parse("rumahl.search.query").unwrap(),
                )
                .into(),
            )
            .unwrap();
        manifest
            .add_event_subscription(EventName::parse("rumahl.files.changed").unwrap())
            .unwrap();

        let mut state = PlatformState::new();
        let installed = AppLifecycle::new().install(manifest, &mut state).unwrap();
        let user = UserIdentity::new(UserId::new());
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(*user.id(), UserRole::User);
        let grant = GrantAuthority::new()
            .issue(
                &policy,
                user.into(),
                installed.identity().clone().into(),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![ResourceRef::new(
                    ResourceNamespace::parse("rumahl.files").unwrap(),
                    ResourceKind::parse("file").unwrap(),
                    ResourceKey::parse("document-1").unwrap(),
                )],
            )
            .unwrap();
        let mut grants = InMemoryGrantStore::new();
        grants.insert(grant);

        PlatformSnapshot::capture(&state, &grants)
    }

    #[test]
    fn empty_repository_has_no_snapshot() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();

        assert!(repository.load().unwrap().is_none());
    }

    #[test]
    fn stores_and_recovers_complete_platform_snapshot() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();
        repository.store(&populated_snapshot()).unwrap();
        let snapshot = repository.load().unwrap().unwrap();
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        let report = PlatformRecovery::new()
            .recover(&snapshot, &mut state, &mut grants)
            .unwrap();

        assert_eq!(report.installed_apps(), 1);
        assert_eq!(report.grants(), 1);
        assert_eq!(state.capability_registry().len(), 1);
        assert_eq!(state.contribution_registry().len(), 2);
        assert_eq!(state.command_registry().len(), 1);
        assert_eq!(state.search_registry().len(), 1);
        assert_eq!(state.event_bus().len(), 1);
        assert_eq!(state.database_registry().len(), 1);
        assert_eq!(
            state
                .database_registry()
                .databases_for_owner(state.installed_apps().apps()[0].identity())[0]
                .id()
                .as_str(),
            "primary"
        );
        let oidc = state.installed_apps().apps()[0]
            .manifest()
            .oidc_client()
            .unwrap();
        assert_eq!(oidc.client_type(), OidcClientType::Public);
        assert_eq!(oidc.callback_path().as_str(), "/oidc/callback");
    }

    #[test]
    fn replaces_singleton_snapshot_atomically() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();
        repository.store(&populated_snapshot()).unwrap();
        repository
            .store(&PlatformSnapshot::new(
                PLATFORM_SNAPSHOT_VERSION,
                Vec::new(),
                Vec::new(),
            ))
            .unwrap();

        let snapshot = repository.load().unwrap().unwrap();

        assert!(snapshot.installed_apps().is_empty());
        assert!(snapshot.grants().is_empty());
    }

    #[test]
    fn rejects_version_mismatch_between_column_and_payload() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();
        repository.store(&populated_snapshot()).unwrap();
        repository
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE platform_snapshot SET snapshot_version = ?1 WHERE id = ?2",
                params![i64::from(PLATFORM_SNAPSHOT_VERSION + 1), SNAPSHOT_ID],
            )
            .unwrap();

        assert!(matches!(
            repository.load(),
            Err(SqliteSnapshotRepositoryError::VersionMismatch { .. })
        ));
    }

    #[test]
    fn rejects_unknown_wire_fields() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();
        repository
            .connection
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO platform_snapshot (id, snapshot_version, payload)
                 VALUES (?1, ?2, ?3)",
                params![
                    SNAPSHOT_ID,
                    i64::from(PLATFORM_SNAPSHOT_VERSION),
                    r#"{"version":1,"installed_apps":[],"grants":[],"unexpected":true}"#
                ],
            )
            .unwrap();

        assert!(matches!(
            repository.load(),
            Err(SqliteSnapshotRepositoryError::Json(_))
        ));
    }

    #[test]
    fn rejects_domain_invalid_wire_values() {
        let repository = SqliteSnapshotRepository::open_in_memory().unwrap();
        repository.store(&populated_snapshot()).unwrap();
        let payload: String = repository
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT payload FROM platform_snapshot WHERE id = ?1",
                [SNAPSHOT_ID],
                |row| row.get(0),
            )
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        value["installed_apps"][0]["identity"]["app_id"] =
            serde_json::Value::String("INVALID".to_owned());
        repository
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE platform_snapshot SET payload = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value).unwrap(), SNAPSHOT_ID],
            )
            .unwrap();

        assert!(matches!(
            repository.load(),
            Err(SqliteSnapshotRepositoryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn snapshot_survives_database_reopen() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rumahl-snapshot-{}-{nonce}.sqlite",
            std::process::id()
        ));

        {
            let repository = SqliteSnapshotRepository::open(&path).unwrap();
            repository.store(&populated_snapshot()).unwrap();
        }

        let reopened = SqliteSnapshotRepository::open(&path).unwrap();
        let snapshot = reopened.load().unwrap().unwrap();

        assert_eq!(snapshot.installed_apps().len(), 1);
        assert_eq!(snapshot.grants().len(), 1);

        drop(reopened);
        fs::remove_file(&path).unwrap();
    }
}
