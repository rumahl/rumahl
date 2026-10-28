//! Presentation preferences, never identity or authorization policy.
use crate::UserId;
use uuid::Uuid;

/// Built-in fallback theme when neither the account nor the browser overrides it.
pub const DEFAULT_THEME_ID: &str = "com.rumahl.default";
const MAX_THEME_ID_BYTES: usize = 128;

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

/// A theme is referenced by its namespaced id; its token data stays in the contract layer.
pub fn valid_theme_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_THEME_ID_BYTES
        && value.is_ascii()
        && value.split('.').count() >= 3
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !segment.ends_with('-')
        })
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

/// A single presentation value update. `None` means "inherit/remove the override".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPreferenceUpdate {
    Mode(Option<ShellMode>),
    Theme(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPreferences {
    pub revision: u64,
    pub user_mode: ShellMode,
    pub device_mode: Option<ShellMode>,
    pub user_theme: String,
    pub device_theme: Option<String>,
}
impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            revision: 0,
            user_mode: ShellMode::Desktop,
            device_mode: None,
            user_theme: DEFAULT_THEME_ID.to_owned(),
            device_theme: None,
        }
    }
}
impl ShellPreferences {
    pub fn effective_mode(&self) -> ShellMode {
        self.device_mode.unwrap_or(self.user_mode)
    }
    pub fn effective_theme(&self) -> &str {
        self.device_theme.as_deref().unwrap_or(&self.user_theme)
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
    /// `None` removes an override (or restores the default at user scope).
    fn save(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        update: ShellPreferenceUpdate,
    ) -> Result<ShellPreferences, ShellPreferencesError>;
}
