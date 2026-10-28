use std::error::Error;

use super::PlatformSnapshot;

pub trait PlatformSnapshotRepository {
    type Error: Error + Send + Sync + 'static;

    fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error>;

    fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error>;
}
