use std::error::Error;

use rumahl_core::{InstallationId, UnixTimestamp};

use crate::{OidcClientId, OidcClientRecord};

pub trait OidcClientRepository {
    type Error: Error + Send + Sync + 'static;

    /// Persists the complete client registration as one atomic operation.
    ///
    /// Implementations must leave the repository unchanged when this method
    /// returns an error.
    fn insert(&self, client: &OidcClientRecord) -> Result<(), Self::Error>;

    fn find_active_by_id(
        &self,
        client_id: &OidcClientId,
    ) -> Result<Option<OidcClientRecord>, Self::Error>;

    fn find_active_by_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<Option<OidcClientRecord>, Self::Error>;

    /// Revokes every active client for an installation atomically.
    ///
    /// Implementations must leave the repository unchanged when this method
    /// returns an error.
    fn revoke_for_installation(
        &self,
        installation_id: &InstallationId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, Self::Error>;
}
