use std::error::Error;
use std::fmt;

use crate::PlatformState;

use super::{
    PlatformRecovery, PlatformRecoveryError, PlatformRecoveryReport, PlatformSnapshot,
    PlatformSnapshotRepository,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformPersistence {
    recovery: PlatformRecovery,
}

#[derive(Debug)]
pub enum PlatformLoadError<E> {
    Repository(E),
    Recovery(PlatformRecoveryError),
}

impl PlatformPersistence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save<R>(&self, state: &PlatformState, repository: &R) -> Result<(), R::Error>
    where
        R: PlatformSnapshotRepository,
    {
        repository.store(&PlatformSnapshot::capture(state))
    }

    pub fn load<R>(
        &self,
        state: &mut PlatformState,
        repository: &R,
    ) -> Result<Option<PlatformRecoveryReport>, PlatformLoadError<R::Error>>
    where
        R: PlatformSnapshotRepository,
    {
        let Some(snapshot) = repository.load().map_err(PlatformLoadError::Repository)? else {
            return Ok(None);
        };

        self.recovery
            .recover(&snapshot, state)
            .map(Some)
            .map_err(PlatformLoadError::Recovery)
    }
}

impl<E> fmt::Display for PlatformLoadError<E>
where
    E: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => {
                write!(f, "cannot load platform snapshot: {error}")
            }

            Self::Recovery(error) => {
                write!(f, "cannot recover platform state: {error}")
            }
        }
    }
}

impl<E> Error for PlatformLoadError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Repository(error) => Some(error),
            Self::Recovery(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::convert::Infallible;

    use super::*;
    use crate::{
        AppId, AppLifecycle, AppManifest, AppVersion, PackagePath, PublisherId, RuntimeDescriptor,
        RuntimeEntrypoint, RuntimeEntrypointId,
    };

    #[derive(Default)]
    struct MemoryRepository {
        snapshot: RefCell<Option<PlatformSnapshot>>,
    }

    impl PlatformSnapshotRepository for MemoryRepository {
        type Error = Infallible;

        fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error> {
            Ok(self.snapshot.borrow().clone())
        }

        fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error> {
            self.snapshot.replace(Some(snapshot.clone()));
            Ok(())
        }
    }

    fn manifest() -> AppManifest {
        let mut runtime = RuntimeDescriptor::web();

        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap()
    }

    #[test]
    fn save_then_load_restores_the_same_installation() {
        let persistence = PlatformPersistence::new();
        let repository = MemoryRepository::default();
        let mut source = PlatformState::new();
        let installed = AppLifecycle::new()
            .install(manifest(), &mut source)
            .unwrap();
        let installation_id = *installed.installation_id();

        persistence.save(&source, &repository).unwrap();

        let mut recovered = PlatformState::new();
        let report = persistence
            .load(&mut recovered, &repository)
            .unwrap()
            .unwrap();

        assert_eq!(report.installed_apps(), 1);
        assert!(
            recovered
                .installed_apps()
                .get_by_installation_id(&installation_id)
                .is_some()
        );
    }

    #[test]
    fn missing_snapshot_keeps_live_state_unchanged() {
        let persistence = PlatformPersistence::new();
        let repository = MemoryRepository::default();
        let mut state = PlatformState::new();
        let installed = AppLifecycle::new().install(manifest(), &mut state).unwrap();
        let installation_id = *installed.installation_id();

        assert!(persistence.load(&mut state, &repository).unwrap().is_none());
        assert!(
            state
                .installed_apps()
                .get_by_installation_id(&installation_id)
                .is_some()
        );
    }
}
