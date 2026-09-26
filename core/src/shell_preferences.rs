//! Presentation preferences, never identity or authorization policy.
use crate::UserId;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellMode {
    Desktop,
    Launcher,
}
impl ShellMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Launcher => "launcher",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "desktop" => Some(Self::Desktop),
            "launcher" => Some(Self::Launcher),
            _ => None,
        }
    }
}
/// A browser-generated storage namespace, NOT a trusted device identity or credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserProfileId(Uuid);
impl BrowserProfileId {
    pub fn parse(value: &str) -> Option<Self> {
        let id = Uuid::parse_str(value).ok()?;
        (id.get_version_num() == 4
            && id.get_variant() == uuid::Variant::RFC4122
            && id.to_string() == value)
            .then_some(Self(id))
    }
}
impl std::fmt::Display for BrowserProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceScope {
    User,
    Device,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellPreferences {
    pub revision: u64,
    pub user_mode: ShellMode,
    pub device_mode: Option<ShellMode>,
}
impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            revision: 0,
            user_mode: ShellMode::Desktop,
            device_mode: None,
        }
    }
}
impl ShellPreferences {
    pub fn effective_mode(self) -> ShellMode {
        self.device_mode.unwrap_or(self.user_mode)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPreferencesError {
    Conflict,
    Unavailable,
    Limit,
}
pub trait ShellPreferencesRepository: Send + Sync + 'static {
    fn load(
        &self,
        user: UserId,
        device: BrowserProfileId,
    ) -> Result<ShellPreferences, ShellPreferencesError>;
    /// Null removes an override (or restores the default at user scope).
    fn save(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        mode: Option<ShellMode>,
    ) -> Result<ShellPreferences, ShellPreferencesError>;
}
