use std::error::Error;

use rumahl_core::{AppOperationId, InstallationId, InstalledApp};
use rumahl_oidc_provider::{OidcClientId, OidcClientSecret};

/// Privileged channel for installation-scoped runtime secret delivery.
///
/// Delivery must be idempotent for the tuple `(operation_id, installation_id,
/// client_id)`. Replaying the same value acknowledges an already completed
/// delivery; replaying a different value for that tuple must fail closed.
/// Implementations must not persist the plaintext in app storage, command-line
/// arguments, logs, or the platform snapshot.
pub trait RuntimeSecretDelivery {
    type Error: Error + Send + Sync + 'static;

    fn deliver_oidc_client_secret(
        &self,
        operation_id: &AppOperationId,
        app: &InstalledApp,
        client_id: &OidcClientId,
        client_secret: &OidcClientSecret,
    ) -> Result<(), Self::Error>;

    /// Removes any not-yet-committed runtime material for an installation.
    /// This operation must be idempotent.
    fn remove_for_installation(
        &self,
        operation_id: &AppOperationId,
        installation_id: &InstallationId,
    ) -> Result<(), Self::Error>;
}
