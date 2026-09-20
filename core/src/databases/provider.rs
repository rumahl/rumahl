use std::error::Error;

use crate::InstallationId;

use super::AppDatabaseBinding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDatabaseInstallationState {
    Absent,
    Staged,
    Active,
    Retained,
}

/// Engine-neutral boundary implemented by an OS database adapter.
///
/// Provider-specific access values, such as an embedded connection or a local
/// runtime credential handle, stay outside the core domain model.
pub trait AppDatabaseProvider {
    type Access;
    type Error: Error + Send + Sync + 'static;

    /// Provisions every declared database for one installation as a unit.
    ///
    /// An empty slice is a no-op. Implementations must reject bindings from
    /// different installations and leave no active partial installation when
    /// this method returns an error. Replaying the same complete binding set
    /// after successful activation must be idempotent and preserve data.
    fn provision_installation(&self, bindings: &[AppDatabaseBinding]) -> Result<(), Self::Error>;

    /// Reports the physical lifecycle state without exposing provider paths.
    fn installation_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppDatabaseInstallationState, Self::Error>;

    /// Resolves provider-specific access for an active database binding.
    fn access(&self, binding: &AppDatabaseBinding) -> Result<Self::Access, Self::Error>;

    /// Makes all databases for an installation inaccessible while retaining
    /// their physical data for explicit recovery or later purge.
    fn retain_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error>;

    /// Reactivates retained data for the exact same installation identity.
    fn restore_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error>;
}
