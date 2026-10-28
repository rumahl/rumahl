use std::error::Error;

use crate::{AppIdentity, InstallationId};

use super::{SecretPurpose, SecretRecord};

pub trait SecretStore {
    type Error: Error + Send + Sync + 'static;

    /// Inserts one encrypted-at-rest secret.
    ///
    /// Implementations must enforce uniqueness for `(InstallationId,
    /// SecretPurpose)` and leave existing material unchanged on conflict.
    fn insert(&self, secret: &SecretRecord) -> Result<(), Self::Error>;

    fn find_by_owner_and_purpose(
        &self,
        owner: &AppIdentity,
        purpose: &SecretPurpose,
    ) -> Result<Option<SecretRecord>, Self::Error>;

    /// Removes all material for an installation as one atomic store operation.
    fn remove_for_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<usize, Self::Error>;
}
