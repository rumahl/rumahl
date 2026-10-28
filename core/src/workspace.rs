use crate::{BrowserProfileId, PreferenceScope, ShellPreferencesError, UserId};
#[derive(Debug, Clone, Default)]
pub struct WorkspacePreferences {
    pub revision: u64,
    pub user: Option<String>,
    pub device: Option<String>,
}
pub trait WorkspaceRepository: Send + Sync {
    fn load_workspace(
        &self,
        user: UserId,
        device: BrowserProfileId,
    ) -> Result<WorkspacePreferences, ShellPreferencesError>;
    fn save_workspace(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        value: Option<String>,
    ) -> Result<WorkspacePreferences, ShellPreferencesError>;
}
