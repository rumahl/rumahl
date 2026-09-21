use std::error::Error;

use crate::{InstallationId, InstalledApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRuntimeInstallationState {
    Absent,
    Prepared,
    Active,
}

/// OS boundary for an installation-scoped app runtime.
///
/// Preparation creates the isolated runtime and its secret namespace without
/// starting app code. Activation occurs only after all declared runtime
/// secrets have been delivered. Every operation must be idempotent so an
/// `applying` journal step can be replayed after a process restart.
pub trait AppRuntimeProvider {
    type Error: Error + Send + Sync + 'static;

    /// Prepares the exact runtime declared by `app`, but does not start it.
    /// Replaying the same installation and declaration must succeed; a
    /// conflicting runtime already using the installation identity must fail.
    fn prepare_installation(&self, app: &InstalledApp) -> Result<(), Self::Error>;

    /// Starts a prepared runtime. Replaying activation of the same runtime is
    /// successful and must not create a second instance.
    fn activate_installation(&self, app: &InstalledApp) -> Result<(), Self::Error>;

    fn installation_state(
        &self,
        installation_id: &InstallationId,
    ) -> Result<AppRuntimeInstallationState, Self::Error>;

    /// Stops an active runtime while retaining its prepared namespace.
    fn deactivate_installation(
        &self,
        installation_id: &InstallationId,
    ) -> Result<bool, Self::Error>;

    /// Removes a stopped/prepared runtime. An absent runtime is an idempotent
    /// no-op; implementations must reject removal of an active runtime.
    fn remove_installation(&self, installation_id: &InstallationId) -> Result<bool, Self::Error>;
}
