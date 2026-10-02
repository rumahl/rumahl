mod event;
mod snapshot;
mod theme;
mod typescript;

pub use snapshot::{
    ExtensionContribution, ExtensionSlot, LauncherLayoutVariant, ShellApp, ShellLayoutVariant,
    ShellMode, ShellSnapshot, ShellSnapshotError, ShellSystemStatus, ShellTheme, ShellUser,
    SystemProtectionStatus, WindowChromeVariant,
};
pub use theme::{
    ResolvedTheme, ThemeManifest, ThemeManifestError, ThemeToken, ThemeTokenValue, theme_by_id,
};
pub use typescript::typescript_contracts_v1;

/// Namespaced id of the built-in rumahl theme (the user-preference default).
pub const DEFAULT_THEME_ID: &str = "com.rumahl.default";
pub const MANIFEST_VERSION: u16 = 1;
pub const UI_CONTRACT_VERSION: u16 = 1;
pub const EXTENSION_API_VERSION: u16 = 1;
pub const SNAPSHOT_VERSION: u16 = 1;
pub use event::{MAX_SHELL_EVENT_BYTES, SHELL_EVENT_VERSION, ShellEvent, ShellEventError};
