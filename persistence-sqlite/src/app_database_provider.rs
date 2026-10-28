use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumahl_core::{
    AppDatabaseBinding, AppDatabaseInstallationState, AppDatabaseProvider, InstallationId,
};
use rusqlite::Connection;

const ACTIVE_DIRECTORY: &str = "active";
const RETAINED_DIRECTORY: &str = "retained";
const STAGING_DIRECTORY: &str = ".staging";

#[derive(Debug)]
pub struct SqliteAppDatabaseProvider {
    root: PathBuf,
}

#[derive(Debug)]
pub enum SqliteAppDatabaseProviderError {
    Io(io::Error),
    Database(rusqlite::Error),
    MixedInstallationBindings,
    DuplicateDatabaseBinding,
    RetainedDataExists,
    StagingDataExists,
    ConflictingInstallationState,
    ActiveDataMismatch,
    DatabaseNotActive,
}

impl SqliteAppDatabaseProvider {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, SqliteAppDatabaseProviderError> {
        fs::create_dir_all(root.as_ref()).map_err(SqliteAppDatabaseProviderError::Io)?;
        let root = fs::canonicalize(root.as_ref()).map_err(SqliteAppDatabaseProviderError::Io)?;
        let provider = Self { root };

        for directory in [ACTIVE_DIRECTORY, RETAINED_DIRECTORY, STAGING_DIRECTORY] {
            fs::create_dir_all(provider.root.join(directory))
                .map_err(SqliteAppDatabaseProviderError::Io)?;
        }

        Ok(provider)
    }

    fn installation_directory(&self, area: &str, installation_id: &InstallationId) -> PathBuf {
        self.root.join(area).join(installation_id.to_string())
    }

    fn active_database_path(&self, binding: &AppDatabaseBinding) -> PathBuf {
        self.installation_directory(ACTIVE_DIRECTORY, binding.owner().installation_id())
            .join(format!("{}.sqlite3", binding.id().as_str()))
    }

    fn installation_state_internal(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppDatabaseInstallationState, SqliteAppDatabaseProviderError> {
        let active = self.installation_directory(ACTIVE_DIRECTORY, installation_id);
        let retained = self.installation_directory(RETAINED_DIRECTORY, installation_id);
        let staging = self.installation_directory(STAGING_DIRECTORY, installation_id);
        let active_exists = active
            .try_exists()
            .map_err(SqliteAppDatabaseProviderError::Io)?;
        let retained_exists = retained
            .try_exists()
            .map_err(SqliteAppDatabaseProviderError::Io)?;
        let staging_exists = staging
            .try_exists()
            .map_err(SqliteAppDatabaseProviderError::Io)?;

        if [active_exists, retained_exists, staging_exists]
            .into_iter()
            .filter(|exists| *exists)
            .count()
            > 1
        {
            return Err(SqliteAppDatabaseProviderError::ConflictingInstallationState);
        }

        Ok(if active_exists {
            AppDatabaseInstallationState::Active
        } else if retained_exists {
            AppDatabaseInstallationState::Retained
        } else if staging_exists {
            AppDatabaseInstallationState::Staged
        } else {
            AppDatabaseInstallationState::Absent
        })
    }

    fn verify_active_bindings(
        &self,
        installation_id: &InstallationId,
        bindings: &[AppDatabaseBinding],
    ) -> Result<(), SqliteAppDatabaseProviderError> {
        let active = self.installation_directory(ACTIVE_DIRECTORY, installation_id);
        let expected = bindings
            .iter()
            .map(|binding| format!("{}.sqlite3", binding.id().as_str()))
            .collect::<HashSet<_>>();
        let mut found = HashSet::new();

        for entry in fs::read_dir(active).map_err(SqliteAppDatabaseProviderError::Io)? {
            let entry = entry.map_err(SqliteAppDatabaseProviderError::Io)?;
            let file_type = entry
                .file_type()
                .map_err(SqliteAppDatabaseProviderError::Io)?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err(SqliteAppDatabaseProviderError::ActiveDataMismatch);
            };

            if !file_type.is_file() {
                return Err(SqliteAppDatabaseProviderError::ActiveDataMismatch);
            }

            if expected.contains(&name) {
                found.insert(name);
                continue;
            }

            let recognized_sidecar = ["-wal", "-shm", "-journal"].iter().any(|suffix| {
                name.strip_suffix(suffix)
                    .is_some_and(|base| expected.contains(base))
            });
            if !recognized_sidecar {
                return Err(SqliteAppDatabaseProviderError::ActiveDataMismatch);
            }
        }

        if found != expected {
            return Err(SqliteAppDatabaseProviderError::ActiveDataMismatch);
        }

        Ok(())
    }

    fn configure(connection: &Connection) -> Result<(), SqliteAppDatabaseProviderError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(SqliteAppDatabaseProviderError::Database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;",
            )
            .map_err(SqliteAppDatabaseProviderError::Database)
    }

    fn validate_bindings(
        bindings: &[AppDatabaseBinding],
    ) -> Result<Option<InstallationId>, SqliteAppDatabaseProviderError> {
        let Some(first) = bindings.first() else {
            return Ok(None);
        };
        let owner = first.owner();
        let mut database_ids = HashSet::new();

        for binding in bindings {
            if binding.owner() != owner {
                return Err(SqliteAppDatabaseProviderError::MixedInstallationBindings);
            }

            if !database_ids.insert(binding.id()) {
                return Err(SqliteAppDatabaseProviderError::DuplicateDatabaseBinding);
            }
        }

        Ok(Some(*owner.installation_id()))
    }

    fn remove_staging(directory: &Path) {
        let _ = fs::remove_dir_all(directory);
    }
}

impl AppDatabaseProvider for SqliteAppDatabaseProvider {
    type Access = Connection;
    type Error = SqliteAppDatabaseProviderError;

    fn provision_installation(&self, bindings: &[AppDatabaseBinding]) -> Result<(), Self::Error> {
        let Some(installation_id) = Self::validate_bindings(bindings)? else {
            return Ok(());
        };
        let active = self.installation_directory(ACTIVE_DIRECTORY, &installation_id);
        let staging = self.installation_directory(STAGING_DIRECTORY, &installation_id);

        match self.installation_state_internal(&installation_id)? {
            AppDatabaseInstallationState::Active => {
                return self.verify_active_bindings(&installation_id, bindings);
            }
            AppDatabaseInstallationState::Retained => {
                return Err(SqliteAppDatabaseProviderError::RetainedDataExists);
            }
            AppDatabaseInstallationState::Staged => {
                fs::remove_dir_all(&staging).map_err(SqliteAppDatabaseProviderError::Io)?
            }
            AppDatabaseInstallationState::Absent => {}
        }

        fs::create_dir(&staging).map_err(SqliteAppDatabaseProviderError::Io)?;

        let provision = (|| {
            for binding in bindings {
                let path = staging.join(format!("{}.sqlite3", binding.id().as_str()));
                let connection =
                    Connection::open(path).map_err(SqliteAppDatabaseProviderError::Database)?;
                Self::configure(&connection)?;
            }

            fs::rename(&staging, &active).map_err(SqliteAppDatabaseProviderError::Io)
        })();

        if provision.is_err() {
            Self::remove_staging(&staging);
        }

        provision
    }

    fn installation_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppDatabaseInstallationState, Self::Error> {
        self.installation_state_internal(installation_id)
    }

    fn access(&self, binding: &AppDatabaseBinding) -> Result<Self::Access, Self::Error> {
        let path = self.active_database_path(binding);

        if !path.is_file() {
            return Err(SqliteAppDatabaseProviderError::DatabaseNotActive);
        }

        let connection =
            Connection::open(path).map_err(SqliteAppDatabaseProviderError::Database)?;
        Self::configure(&connection)?;

        Ok(connection)
    }

    fn retain_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error> {
        let active = self.installation_directory(ACTIVE_DIRECTORY, installation_id);
        let retained = self.installation_directory(RETAINED_DIRECTORY, installation_id);

        match self.installation_state_internal(installation_id)? {
            AppDatabaseInstallationState::Absent => return Ok(false),
            AppDatabaseInstallationState::Retained => return Ok(true),
            AppDatabaseInstallationState::Staged => {
                return Err(SqliteAppDatabaseProviderError::StagingDataExists);
            }
            AppDatabaseInstallationState::Active => {}
        }

        fs::rename(active, retained).map_err(SqliteAppDatabaseProviderError::Io)?;

        Ok(true)
    }

    fn restore_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error> {
        let retained = self.installation_directory(RETAINED_DIRECTORY, installation_id);
        let active = self.installation_directory(ACTIVE_DIRECTORY, installation_id);

        match self.installation_state_internal(installation_id)? {
            AppDatabaseInstallationState::Absent => return Ok(false),
            AppDatabaseInstallationState::Active => return Ok(true),
            AppDatabaseInstallationState::Staged => {
                return Err(SqliteAppDatabaseProviderError::StagingDataExists);
            }
            AppDatabaseInstallationState::Retained => {}
        }

        fs::rename(retained, active).map_err(SqliteAppDatabaseProviderError::Io)?;

        Ok(true)
    }
}

impl fmt::Display for SqliteAppDatabaseProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => write!(f, "SQLite app database filesystem operation failed"),
            Self::Database(_) => write!(f, "SQLite app database operation failed"),
            Self::MixedInstallationBindings => {
                write!(f, "app database bindings must belong to one installation")
            }
            Self::DuplicateDatabaseBinding => {
                write!(f, "app database bindings contain a duplicate logical id")
            }
            Self::RetainedDataExists => {
                write!(f, "retained app database data already exists")
            }
            Self::StagingDataExists => {
                write!(f, "staged app database data already exists")
            }
            Self::ConflictingInstallationState => {
                write!(
                    f,
                    "app database installation has conflicting physical states"
                )
            }
            Self::ActiveDataMismatch => {
                write!(f, "active app databases do not match the declared bindings")
            }
            Self::DatabaseNotActive => write!(f, "app database is not active"),
        }
    }
}

impl Error for SqliteAppDatabaseProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::MixedInstallationBindings
            | Self::DuplicateDatabaseBinding
            | Self::RetainedDataExists
            | Self::StagingDataExists
            | Self::ConflictingInstallationState
            | Self::ActiveDataMismatch
            | Self::DatabaseNotActive => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use rumahl_core::{AppDatabaseDeclaration, AppDatabaseId, AppId, AppIdentity, PublisherId};

    use super::*;

    fn provider_root(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("rumahl-{test_name}-{nonce}"))
    }

    fn owner(app_id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(app_id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn binding(owner: &AppIdentity, database_id: &str) -> AppDatabaseBinding {
        AppDatabaseBinding::new(
            owner.clone(),
            AppDatabaseDeclaration::new(AppDatabaseId::parse(database_id).unwrap()),
        )
    }

    #[test]
    fn provisions_isolated_databases_and_preserves_data_across_retain_restore() {
        let root = provider_root("app-database-provider");
        let provider = SqliteAppDatabaseProvider::open(&root).unwrap();
        let owner = owner("com.rumahl.notes");
        let primary = binding(&owner, "primary");
        let search = binding(&owner, "search-index");

        provider
            .provision_installation(&[primary.clone(), search.clone()])
            .unwrap();
        {
            let connection = provider.access(&primary).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE note (id INTEGER PRIMARY KEY, title TEXT NOT NULL);
                     INSERT INTO note (title) VALUES ('First note');",
                )
                .unwrap();
        }

        provider
            .provision_installation(&[primary.clone(), search.clone()])
            .unwrap();

        let search_connection = provider.access(&search).unwrap();
        assert!(search_connection.prepare("SELECT * FROM note").is_err());
        drop(search_connection);

        assert!(
            provider
                .retain_installation(owner.installation_id())
                .unwrap()
        );
        assert_eq!(
            provider
                .installation_state(owner.installation_id())
                .unwrap(),
            AppDatabaseInstallationState::Retained
        );
        assert!(
            provider
                .retain_installation(owner.installation_id())
                .unwrap()
        );
        assert!(matches!(
            provider.access(&primary),
            Err(SqliteAppDatabaseProviderError::DatabaseNotActive)
        ));
        assert!(
            provider
                .restore_installation(owner.installation_id())
                .unwrap()
        );
        assert!(
            provider
                .restore_installation(owner.installation_id())
                .unwrap()
        );

        let restored = provider.access(&primary).unwrap();
        let count: i64 = restored
            .query_row("SELECT COUNT(*) FROM note", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        drop(restored);
        drop(provider);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_mixed_installations_without_creating_active_data() {
        let root = provider_root("mixed-app-databases");
        let provider = SqliteAppDatabaseProvider::open(&root).unwrap();
        let notes = owner("com.rumahl.notes");
        let tasks = owner("com.rumahl.tasks");

        assert!(matches!(
            provider
                .provision_installation(&[binding(&notes, "primary"), binding(&tasks, "primary"),]),
            Err(SqliteAppDatabaseProviderError::MixedInstallationBindings)
        ));
        assert!(
            !provider
                .installation_directory(ACTIVE_DIRECTORY, notes.installation_id())
                .exists()
        );
        assert!(
            !provider
                .installation_directory(ACTIVE_DIRECTORY, tasks.installation_id())
                .exists()
        );

        drop(provider);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_declaration_set_has_no_filesystem_side_effect() {
        let root = provider_root("empty-app-databases");
        let provider = SqliteAppDatabaseProvider::open(&root).unwrap();

        provider.provision_installation(&[]).unwrap();

        assert!(
            fs::read_dir(root.join(ACTIVE_DIRECTORY))
                .unwrap()
                .next()
                .is_none()
        );

        drop(provider);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_incomplete_staging_before_replaying_provision() {
        let root = provider_root("staged-app-databases");
        let provider = SqliteAppDatabaseProvider::open(&root).unwrap();
        let owner = owner("com.rumahl.notes");
        let primary = binding(&owner, "primary");
        let staging = provider.installation_directory(STAGING_DIRECTORY, owner.installation_id());
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("partial.sqlite3"), b"incomplete").unwrap();

        provider
            .provision_installation(std::slice::from_ref(&primary))
            .unwrap();

        assert_eq!(
            provider
                .installation_state(owner.installation_id())
                .unwrap(),
            AppDatabaseInstallationState::Active
        );
        assert!(provider.access(&primary).is_ok());

        drop(provider);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_conflicting_or_mismatched_active_state() {
        let root = provider_root("conflicting-app-databases");
        let provider = SqliteAppDatabaseProvider::open(&root).unwrap();
        let owner = owner("com.rumahl.notes");
        let primary = binding(&owner, "primary");
        provider
            .provision_installation(std::slice::from_ref(&primary))
            .unwrap();
        let active = provider.installation_directory(ACTIVE_DIRECTORY, owner.installation_id());
        fs::write(active.join("unexpected.sqlite3"), b"not declared").unwrap();

        assert!(matches!(
            provider.provision_installation(std::slice::from_ref(&primary)),
            Err(SqliteAppDatabaseProviderError::ActiveDataMismatch)
        ));

        fs::remove_file(active.join("unexpected.sqlite3")).unwrap();
        fs::create_dir(
            provider.installation_directory(RETAINED_DIRECTORY, owner.installation_id()),
        )
        .unwrap();
        assert!(matches!(
            provider.installation_state(owner.installation_id()),
            Err(SqliteAppDatabaseProviderError::ConflictingInstallationState)
        ));

        drop(provider);
        fs::remove_dir_all(root).unwrap();
    }
}
