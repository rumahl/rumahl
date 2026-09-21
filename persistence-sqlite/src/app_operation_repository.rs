use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rumahl_core::{
    AppOperation, AppOperationError, AppOperationId, AppOperationIdError, AppOperationKind,
    AppOperationPhase, AppOperationRepository, AppOperationResource, AppOperationResourceState,
    InstallationId, InstallationIdError, UnixTimestamp,
};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};

use crate::wire::WireInstalledApp;

#[derive(Debug)]
pub struct SqliteAppOperationRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteAppOperationRepositoryError {
    Database(rusqlite::Error),
    LockPoisoned,
    AlreadyExists,
    InitialRevisionMustBeZero(u64),
    StaleTransition,
    ValueOutsideSqliteRange { field: &'static str, value: u64 },
    NegativeStoredValue { field: &'static str, value: i64 },
    InvalidOperationId(AppOperationIdError),
    InvalidInstallationId(InstallationIdError),
    InvalidKind(String),
    InvalidPhase(String),
    InvalidResource(String),
    InvalidResourceState(String),
    InvalidStepPosition { expected: usize, actual: i64 },
    Json(serde_json::Error),
    InvalidTargetApp(String),
    InvalidOperation(AppOperationError),
}

impl SqliteAppOperationRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteAppOperationRepositoryError> {
        Self::from_connection(Connection::open(path).map_err(Self::database_error)?)
    }

    pub fn open_in_memory() -> Result<Self, SqliteAppOperationRepositoryError> {
        Self::from_connection(Connection::open_in_memory().map_err(Self::database_error)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteAppOperationRepositoryError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(Self::database_error)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;

                 CREATE TABLE IF NOT EXISTS app_operation (
                     operation_id TEXT PRIMARY KEY,
                     installation_id TEXT NOT NULL,
                     kind TEXT NOT NULL CHECK (kind IN ('install', 'update', 'uninstall')),
                     phase TEXT NOT NULL CHECK (
                         phase IN ('applying', 'committed', 'compensating', 'compensated')
                     ),
                     revision INTEGER NOT NULL CHECK (revision >= 0),
                     started_at INTEGER NOT NULL CHECK (started_at >= 0),
                     updated_at INTEGER NOT NULL CHECK (updated_at >= started_at),
                     target_app TEXT
                 );

                 CREATE TABLE IF NOT EXISTS app_operation_step (
                     operation_id TEXT NOT NULL REFERENCES app_operation(operation_id)
                         ON DELETE CASCADE,
                     position INTEGER NOT NULL CHECK (position >= 0),
                     resource TEXT NOT NULL CHECK (
                         resource IN ('app-databases', 'oidc-client', 'platform-snapshot')
                     ),
                     state TEXT NOT NULL CHECK (
                         state IN (
                             'pending', 'applying', 'applied', 'compensation-pending',
                             'compensating', 'compensated'
                         )
                     ),
                     PRIMARY KEY (operation_id, position),
                     UNIQUE (operation_id, resource)
                 );

                 CREATE INDEX IF NOT EXISTS app_operation_phase
                 ON app_operation(phase, started_at);",
            )
            .map_err(Self::database_error)?;

        let target_app_column_exists = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM pragma_table_info('app_operation') WHERE name = 'target_app'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(Self::database_error)?;
        if !target_app_column_exists {
            connection
                .execute("ALTER TABLE app_operation ADD COLUMN target_app TEXT", [])
                .map_err(Self::database_error)?;
        }

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, SqliteAppOperationRepositoryError> {
        self.connection
            .lock()
            .map_err(|_| SqliteAppOperationRepositoryError::LockPoisoned)
    }

    fn database_error(error: rusqlite::Error) -> SqliteAppOperationRepositoryError {
        SqliteAppOperationRepositoryError::Database(error)
    }

    fn insert_steps(
        transaction: &Transaction<'_>,
        operation: &AppOperation,
    ) -> Result<(), SqliteAppOperationRepositoryError> {
        for (position, step) in operation.steps().iter().enumerate() {
            let position = i64::try_from(position).map_err(|_| {
                SqliteAppOperationRepositoryError::ValueOutsideSqliteRange {
                    field: "step position",
                    value: position as u64,
                }
            })?;

            transaction
                .execute(
                    "INSERT INTO app_operation_step (
                         operation_id, position, resource, state
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        operation.id().to_string(),
                        position,
                        resource_name(step.resource()),
                        resource_state_name(step.state()),
                    ],
                )
                .map_err(Self::database_error)?;
        }

        Ok(())
    }

    fn load_by_id(
        connection: &Connection,
        id: &AppOperationId,
    ) -> Result<Option<AppOperation>, SqliteAppOperationRepositoryError> {
        let row = connection
            .query_row(
                "SELECT operation_id, installation_id, kind, phase, revision, started_at,
                        updated_at, target_app
                 FROM app_operation
                 WHERE operation_id = ?1",
                [id.to_string()],
                |row| {
                    Ok(StoredOperation {
                        operation_id: row.get(0)?,
                        installation_id: row.get(1)?,
                        kind: row.get(2)?,
                        phase: row.get(3)?,
                        revision: row.get(4)?,
                        started_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        target_app: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Self::database_error)?;

        let Some(row) = row else {
            return Ok(None);
        };

        Self::decode(connection, row).map(Some)
    }

    fn decode(
        connection: &Connection,
        stored: StoredOperation,
    ) -> Result<AppOperation, SqliteAppOperationRepositoryError> {
        let mut statement = connection
            .prepare(
                "SELECT position, resource, state
                 FROM app_operation_step
                 WHERE operation_id = ?1
                 ORDER BY position ASC",
            )
            .map_err(Self::database_error)?;
        let rows = statement
            .query_map([stored.operation_id.as_str()], |row| {
                Ok(StoredStep {
                    position: row.get(0)?,
                    resource: row.get(1)?,
                    state: row.get(2)?,
                })
            })
            .map_err(Self::database_error)?;
        let mut steps = Vec::new();

        for (expected_position, row) in rows.enumerate() {
            let row = row.map_err(Self::database_error)?;
            if row.position != expected_position as i64 {
                return Err(SqliteAppOperationRepositoryError::InvalidStepPosition {
                    expected: expected_position,
                    actual: row.position,
                });
            }

            steps.push(AppOperation::restored_step(
                parse_resource(&row.resource)?,
                parse_resource_state(&row.state)?,
            ));
        }

        let target_app = stored
            .target_app
            .map(|payload| {
                serde_json::from_str::<WireInstalledApp>(&payload)
                    .map_err(SqliteAppOperationRepositoryError::Json)?
                    .into_domain()
                    .map_err(|error| {
                        SqliteAppOperationRepositoryError::InvalidTargetApp(error.to_string())
                    })
            })
            .transpose()?;
        let id = AppOperationId::parse(&stored.operation_id)
            .map_err(SqliteAppOperationRepositoryError::InvalidOperationId)?;
        let installation_id = InstallationId::parse(&stored.installation_id)
            .map_err(SqliteAppOperationRepositoryError::InvalidInstallationId)?;
        let kind = parse_kind(&stored.kind)?;
        let phase = parse_phase(&stored.phase)?;
        let revision = from_sql_integer("revision", stored.revision)?;
        let started_at =
            UnixTimestamp::from_seconds(from_sql_integer("started_at", stored.started_at)?);
        let updated_at =
            UnixTimestamp::from_seconds(from_sql_integer("updated_at", stored.updated_at)?);

        match target_app {
            Some(target_app) => AppOperation::restore_for_app(
                id,
                installation_id,
                kind,
                phase,
                steps,
                revision,
                started_at,
                updated_at,
                target_app,
            ),
            None => AppOperation::restore(
                id,
                installation_id,
                kind,
                phase,
                steps,
                revision,
                started_at,
                updated_at,
            ),
        }
        .map_err(SqliteAppOperationRepositoryError::InvalidOperation)
    }
}

impl AppOperationRepository for SqliteAppOperationRepository {
    type Error = SqliteAppOperationRepositoryError;

    fn create(&self, operation: &AppOperation) -> Result<(), Self::Error> {
        if operation.revision() != 0 {
            return Err(
                SqliteAppOperationRepositoryError::InitialRevisionMustBeZero(operation.revision()),
            );
        }

        let revision = to_sql_integer("revision", operation.revision())?;
        let started_at = to_sql_integer("started_at", operation.started_at().as_seconds())?;
        let updated_at = to_sql_integer("updated_at", operation.updated_at().as_seconds())?;
        let target_app = operation
            .target_app()
            .map(|target| {
                serde_json::to_string(&WireInstalledApp::capture(target))
                    .map_err(SqliteAppOperationRepositoryError::Json)
            })
            .transpose()?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;
        let inserted = transaction.execute(
            "INSERT INTO app_operation (
                 operation_id, installation_id, kind, phase, revision, started_at, updated_at,
                 target_app
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                operation.id().to_string(),
                operation.installation_id().to_string(),
                kind_name(operation.kind()),
                phase_name(operation.phase()),
                revision,
                started_at,
                updated_at,
                target_app,
            ],
        );

        match inserted {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation =>
            {
                return Err(SqliteAppOperationRepositoryError::AlreadyExists);
            }
            Err(error) => return Err(Self::database_error(error)),
        }

        Self::insert_steps(&transaction, operation)?;
        transaction.commit().map_err(Self::database_error)
    }

    fn store_transition(&self, operation: &AppOperation) -> Result<(), Self::Error> {
        let Some(previous_revision) = operation.revision().checked_sub(1) else {
            return Err(SqliteAppOperationRepositoryError::StaleTransition);
        };
        let revision = to_sql_integer("revision", operation.revision())?;
        let previous_revision = to_sql_integer("previous revision", previous_revision)?;
        let started_at = to_sql_integer("started_at", operation.started_at().as_seconds())?;
        let updated_at = to_sql_integer("updated_at", operation.updated_at().as_seconds())?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::database_error)?;
        let updated = transaction
            .execute(
                "UPDATE app_operation
                 SET phase = ?1, revision = ?2, updated_at = ?3
                 WHERE operation_id = ?4
                   AND installation_id = ?5
                   AND kind = ?6
                   AND started_at = ?7
                   AND revision = ?8",
                params![
                    phase_name(operation.phase()),
                    revision,
                    updated_at,
                    operation.id().to_string(),
                    operation.installation_id().to_string(),
                    kind_name(operation.kind()),
                    started_at,
                    previous_revision,
                ],
            )
            .map_err(Self::database_error)?;

        if updated != 1 {
            return Err(SqliteAppOperationRepositoryError::StaleTransition);
        }

        transaction
            .execute(
                "DELETE FROM app_operation_step WHERE operation_id = ?1",
                [operation.id().to_string()],
            )
            .map_err(Self::database_error)?;
        Self::insert_steps(&transaction, operation)?;
        transaction.commit().map_err(Self::database_error)
    }

    fn find(&self, id: &AppOperationId) -> Result<Option<AppOperation>, Self::Error> {
        let connection = self.connection()?;

        Self::load_by_id(&connection, id)
    }

    fn list_incomplete(&self) -> Result<Vec<AppOperation>, Self::Error> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT operation_id
                 FROM app_operation
                 WHERE phase IN ('applying', 'compensating')
                 ORDER BY started_at ASC, operation_id ASC",
            )
            .map_err(Self::database_error)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(Self::database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Self::database_error)?;
        drop(statement);

        ids.into_iter()
            .map(|id| {
                let id = AppOperationId::parse(&id)
                    .map_err(SqliteAppOperationRepositoryError::InvalidOperationId)?;
                Self::load_by_id(&connection, &id)?.ok_or(
                    SqliteAppOperationRepositoryError::InvalidOperation(
                        AppOperationError::InvalidRestoredState,
                    ),
                )
            })
            .collect()
    }
}

struct StoredOperation {
    operation_id: String,
    installation_id: String,
    kind: String,
    phase: String,
    revision: i64,
    started_at: i64,
    updated_at: i64,
    target_app: Option<String>,
}

struct StoredStep {
    position: i64,
    resource: String,
    state: String,
}

fn kind_name(kind: AppOperationKind) -> &'static str {
    match kind {
        AppOperationKind::Install => "install",
        AppOperationKind::Update => "update",
        AppOperationKind::Uninstall => "uninstall",
    }
}

fn parse_kind(value: &str) -> Result<AppOperationKind, SqliteAppOperationRepositoryError> {
    match value {
        "install" => Ok(AppOperationKind::Install),
        "update" => Ok(AppOperationKind::Update),
        "uninstall" => Ok(AppOperationKind::Uninstall),
        value => Err(SqliteAppOperationRepositoryError::InvalidKind(
            value.to_owned(),
        )),
    }
}

fn phase_name(phase: AppOperationPhase) -> &'static str {
    match phase {
        AppOperationPhase::Applying => "applying",
        AppOperationPhase::Committed => "committed",
        AppOperationPhase::Compensating => "compensating",
        AppOperationPhase::Compensated => "compensated",
    }
}

fn parse_phase(value: &str) -> Result<AppOperationPhase, SqliteAppOperationRepositoryError> {
    match value {
        "applying" => Ok(AppOperationPhase::Applying),
        "committed" => Ok(AppOperationPhase::Committed),
        "compensating" => Ok(AppOperationPhase::Compensating),
        "compensated" => Ok(AppOperationPhase::Compensated),
        value => Err(SqliteAppOperationRepositoryError::InvalidPhase(
            value.to_owned(),
        )),
    }
}

fn resource_name(resource: AppOperationResource) -> &'static str {
    match resource {
        AppOperationResource::AppDatabases => "app-databases",
        AppOperationResource::OidcClient => "oidc-client",
        AppOperationResource::PlatformSnapshot => "platform-snapshot",
    }
}

fn parse_resource(value: &str) -> Result<AppOperationResource, SqliteAppOperationRepositoryError> {
    match value {
        "app-databases" => Ok(AppOperationResource::AppDatabases),
        "oidc-client" => Ok(AppOperationResource::OidcClient),
        "platform-snapshot" => Ok(AppOperationResource::PlatformSnapshot),
        value => Err(SqliteAppOperationRepositoryError::InvalidResource(
            value.to_owned(),
        )),
    }
}

fn resource_state_name(state: AppOperationResourceState) -> &'static str {
    match state {
        AppOperationResourceState::Pending => "pending",
        AppOperationResourceState::Applying => "applying",
        AppOperationResourceState::Applied => "applied",
        AppOperationResourceState::CompensationPending => "compensation-pending",
        AppOperationResourceState::Compensating => "compensating",
        AppOperationResourceState::Compensated => "compensated",
    }
}

fn parse_resource_state(
    value: &str,
) -> Result<AppOperationResourceState, SqliteAppOperationRepositoryError> {
    match value {
        "pending" => Ok(AppOperationResourceState::Pending),
        "applying" => Ok(AppOperationResourceState::Applying),
        "applied" => Ok(AppOperationResourceState::Applied),
        "compensation-pending" => Ok(AppOperationResourceState::CompensationPending),
        "compensating" => Ok(AppOperationResourceState::Compensating),
        "compensated" => Ok(AppOperationResourceState::Compensated),
        value => Err(SqliteAppOperationRepositoryError::InvalidResourceState(
            value.to_owned(),
        )),
    }
}

fn to_sql_integer(
    field: &'static str,
    value: u64,
) -> Result<i64, SqliteAppOperationRepositoryError> {
    i64::try_from(value)
        .map_err(|_| SqliteAppOperationRepositoryError::ValueOutsideSqliteRange { field, value })
}

fn from_sql_integer(
    field: &'static str,
    value: i64,
) -> Result<u64, SqliteAppOperationRepositoryError> {
    u64::try_from(value)
        .map_err(|_| SqliteAppOperationRepositoryError::NegativeStoredValue { field, value })
}

impl fmt::Display for SqliteAppOperationRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite app operation journal failed: {error}"),
            Self::LockPoisoned => write!(f, "app operation journal lock is poisoned"),
            Self::AlreadyExists => write!(f, "app operation already exists"),
            Self::InitialRevisionMustBeZero(revision) => write!(
                f,
                "new app operation revision must be zero, found {revision}"
            ),
            Self::StaleTransition => write!(f, "app operation transition is stale or missing"),
            Self::ValueOutsideSqliteRange { field, value } => {
                write!(f, "app operation {field} {value} is outside SQLite range")
            }
            Self::NegativeStoredValue { field, value } => {
                write!(f, "stored app operation {field} {value} is negative")
            }
            Self::InvalidOperationId(error) => write!(f, "stored operation ID is invalid: {error}"),
            Self::InvalidInstallationId(error) => {
                write!(f, "stored installation ID is invalid: {error}")
            }
            Self::InvalidKind(value) => write!(f, "stored app operation kind {value:?} is invalid"),
            Self::InvalidPhase(value) => {
                write!(f, "stored app operation phase {value:?} is invalid")
            }
            Self::InvalidResource(value) => {
                write!(f, "stored app operation resource {value:?} is invalid")
            }
            Self::InvalidResourceState(value) => write!(
                f,
                "stored app operation resource state {value:?} is invalid"
            ),
            Self::InvalidStepPosition { expected, actual } => write!(
                f,
                "stored app operation step position must be {expected}, found {actual}"
            ),
            Self::Json(error) => write!(f, "app operation target JSON is invalid: {error}"),
            Self::InvalidTargetApp(error) => {
                write!(f, "stored app operation target is invalid: {error}")
            }
            Self::InvalidOperation(error) => write!(f, "stored app operation is invalid: {error}"),
        }
    }
}

impl Error for SqliteAppOperationRepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidOperationId(error) => Some(error),
            Self::InvalidInstallationId(error) => Some(error),
            Self::InvalidOperation(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::LockPoisoned
            | Self::AlreadyExists
            | Self::InitialRevisionMustBeZero(_)
            | Self::StaleTransition
            | Self::ValueOutsideSqliteRange { .. }
            | Self::NegativeStoredValue { .. }
            | Self::InvalidKind(_)
            | Self::InvalidPhase(_)
            | Self::InvalidResource(_)
            | Self::InvalidResourceState(_)
            | Self::InvalidStepPosition { .. }
            | Self::InvalidTargetApp(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use rumahl_core::{
        AppId, AppLifecycle, AppManifest, AppVersion, PackagePath, PlatformState, PublisherId,
        RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    fn timestamp(value: u64) -> UnixTimestamp {
        UnixTimestamp::from_seconds(value)
    }

    fn operation() -> AppOperation {
        AppOperation::new(
            InstallationId::new(),
            AppOperationKind::Install,
            vec![
                AppOperationResource::AppDatabases,
                AppOperationResource::OidcClient,
                AppOperationResource::PlatformSnapshot,
            ],
            timestamp(10),
        )
        .unwrap()
    }

    fn database_path(test_name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rumahl-{test_name}-{nonce}.sqlite3"))
    }

    fn install_operation() -> AppOperation {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap();
        let app = AppLifecycle::new()
            .install(manifest, &mut PlatformState::new())
            .unwrap();

        AppOperation::for_app(&app, AppOperationKind::Install, timestamp(10)).unwrap()
    }

    #[test]
    fn persists_in_progress_step_for_recovery_after_reopen() {
        let path = database_path("app-operation-journal");
        let mut operation = operation();
        let id = *operation.id();

        {
            let repository = SqliteAppOperationRepository::open(&path).unwrap();
            repository.create(&operation).unwrap();
            operation
                .begin_resource(AppOperationResource::AppDatabases, timestamp(11))
                .unwrap();
            repository.store_transition(&operation).unwrap();
        }

        let repository = SqliteAppOperationRepository::open(&path).unwrap();
        let recovered = repository.find(&id).unwrap().unwrap();
        assert_eq!(recovered, operation);
        assert_eq!(repository.list_incomplete().unwrap(), vec![operation]);

        drop(repository);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn persists_install_target_for_process_restart_recovery() {
        let path = database_path("app-operation-target");
        let operation = install_operation();
        let id = *operation.id();

        {
            let repository = SqliteAppOperationRepository::open(&path).unwrap();
            repository.create(&operation).unwrap();
        }

        let repository = SqliteAppOperationRepository::open(&path).unwrap();
        let recovered = repository.find(&id).unwrap().unwrap();

        assert_eq!(recovered, operation);
        assert_eq!(
            recovered.target_app().unwrap().installation_id(),
            operation.installation_id()
        );

        drop(repository);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn adds_target_column_to_existing_operation_journal() {
        let path = database_path("app-operation-target-migration");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE app_operation (
                         operation_id TEXT PRIMARY KEY,
                         installation_id TEXT NOT NULL,
                         kind TEXT NOT NULL CHECK (kind IN ('install', 'update', 'uninstall')),
                         phase TEXT NOT NULL CHECK (
                             phase IN ('applying', 'committed', 'compensating', 'compensated')
                         ),
                         revision INTEGER NOT NULL CHECK (revision >= 0),
                         started_at INTEGER NOT NULL CHECK (started_at >= 0),
                         updated_at INTEGER NOT NULL CHECK (updated_at >= started_at)
                     );",
                )
                .unwrap();
        }

        drop(SqliteAppOperationRepository::open(&path).unwrap());

        let connection = Connection::open(&path).unwrap();
        let target_columns = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM pragma_table_info('app_operation')
                 WHERE name = 'target_app'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(target_columns, 1);

        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn committed_operations_are_not_listed_as_incomplete() {
        let repository = SqliteAppOperationRepository::open_in_memory().unwrap();
        let mut operation = AppOperation::new(
            InstallationId::new(),
            AppOperationKind::Install,
            vec![AppOperationResource::PlatformSnapshot],
            timestamp(10),
        )
        .unwrap();
        repository.create(&operation).unwrap();
        operation
            .begin_resource(AppOperationResource::PlatformSnapshot, timestamp(11))
            .unwrap();
        repository.store_transition(&operation).unwrap();
        operation
            .complete_resource(AppOperationResource::PlatformSnapshot, timestamp(12))
            .unwrap();
        repository.store_transition(&operation).unwrap();
        operation.commit(timestamp(13)).unwrap();
        repository.store_transition(&operation).unwrap();

        assert!(repository.list_incomplete().unwrap().is_empty());
        assert_eq!(
            repository.find(operation.id()).unwrap().unwrap().phase(),
            AppOperationPhase::Committed
        );
    }

    #[test]
    fn rejects_stale_concurrent_transition() {
        let repository = SqliteAppOperationRepository::open_in_memory().unwrap();
        let mut first = operation();
        let mut stale = first.clone();
        repository.create(&first).unwrap();

        first
            .begin_resource(AppOperationResource::AppDatabases, timestamp(11))
            .unwrap();
        repository.store_transition(&first).unwrap();

        stale
            .begin_resource(AppOperationResource::AppDatabases, timestamp(12))
            .unwrap();
        assert!(matches!(
            repository.store_transition(&stale),
            Err(SqliteAppOperationRepositoryError::StaleTransition)
        ));
        assert_eq!(repository.find(first.id()).unwrap().unwrap(), first);
    }

    #[test]
    fn compensation_state_round_trips() {
        let repository = SqliteAppOperationRepository::open_in_memory().unwrap();
        let mut operation = operation();
        repository.create(&operation).unwrap();
        operation
            .begin_resource(AppOperationResource::AppDatabases, timestamp(11))
            .unwrap();
        repository.store_transition(&operation).unwrap();
        operation.begin_compensation(timestamp(12)).unwrap();
        repository.store_transition(&operation).unwrap();

        let restored = repository.find(operation.id()).unwrap().unwrap();

        assert_eq!(restored, operation);
        assert_eq!(restored.phase(), AppOperationPhase::Compensating);
        assert_eq!(
            restored.steps()[0].state(),
            AppOperationResourceState::CompensationPending
        );
    }
}
