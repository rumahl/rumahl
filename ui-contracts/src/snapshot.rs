use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::theme::{ResolvedTheme, is_known_token_id, valid_token_value};
use crate::{EXTENSION_API_VERSION, SNAPSHOT_VERSION, UI_CONTRACT_VERSION};

const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
const MAX_BUILD_ID_BYTES: usize = 128;
const MAX_REVISION_BYTES: usize = 128;
const MAX_DISPLAY_NAME_CHARS: usize = 128;
const MAX_APPS: usize = 256;
const MAX_WORKSPACE_BYTES: usize = 96 * 1024;
const MAX_CONTRIBUTIONS: usize = 512;
const MAX_JAVASCRIPT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowChromeVariant {
    Standard,
    Compact,
}

/// Structure of the desktop shell surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellLayoutVariant {
    Dock,
    Taskbar,
}

/// Structure of the full-screen launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LauncherLayoutVariant {
    Springboard,
    Drawer,
}

/// Effective presentation mode resolved for the account/device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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

impl ShellLayoutVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dock => "dock",
            Self::Taskbar => "taskbar",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "dock" => Some(Self::Dock),
            "taskbar" => Some(Self::Taskbar),
            _ => None,
        }
    }
}

impl LauncherLayoutVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Springboard => "springboard",
            Self::Drawer => "drawer",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "springboard" => Some(Self::Springboard),
            "drawer" => Some(Self::Drawer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemProtectionStatus {
    Active,
    Attention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellUser {
    display_name: String,
    locale: String,
}

/// An authorized installed app the shell may display without a client fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellApp {
    id: String,
    title: String,
    launchable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellTheme {
    id: String,
    stylesheet_url: String,
    window_chrome: WindowChromeVariant,
    shell_layout: ShellLayoutVariant,
    launcher_layout: LauncherLayoutVariant,
    tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSystemStatus {
    protection: SystemProtectionStatus,
    installed_app_count: u32,
    observed_at_unix_ms: u64,
    last_activity_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSlot {
    DashboardWidgets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExtensionContribution {
    Command {
        id: String,
        title: String,
        capability: String,
    },
    SearchProvider {
        id: String,
        title: String,
        capability: String,
    },
    Widget {
        id: String,
        title: String,
        slot: ExtensionSlot,
        entrypoint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    snapshot_version: u16,
    ui_contract_version: u16,
    extension_api_version: u16,
    shell_build_id: String,
    revision: String,
    user: ShellUser,
    theme: ShellTheme,
    apps: Vec<ShellApp>,
    workspace: Option<String>,
    system_status: ShellSystemStatus,
    mode: ShellMode,
    contributions: Vec<ExtensionContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellSnapshotError {
    TooLarge,
    InvalidJson,
    UnsupportedSnapshotVersion(u16),
    UnsupportedUiContractVersion(u16),
    UnsupportedExtensionApiVersion(u16),
    InvalidBuildId,
    InvalidRevision,
    InvalidDisplayName,
    InvalidLocale,
    InvalidThemeId,
    InvalidStylesheetUrl,
    InvalidThemeTokens,
    InvalidSystemStatus,
    InvalidAppId,
    InvalidAppTitle,
    TooManyApps,
    DuplicateApp,
    InvalidWorkspace,
    TooManyContributions,
    InvalidContributionId,
    InvalidContributionTitle,
    InvalidCapability,
    InvalidEntrypoint,
    DuplicateContribution,
    Serialize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireShellSnapshot {
    snapshot_version: u16,
    ui_contract_version: u16,
    extension_api_version: u16,
    shell_build_id: String,
    revision: String,
    user: WireShellUser,
    theme: WireShellTheme,
    #[serde(default)]
    apps: Vec<WireShellApp>,
    #[serde(default)]
    workspace: Option<String>,
    system_status: WireShellSystemStatus,
    #[serde(default = "default_mode")]
    mode: String,
    contributions: Vec<WireExtensionContribution>,
}

fn default_mode() -> String {
    "desktop".to_owned()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireShellApp {
    id: String,
    title: String,
    launchable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireShellUser {
    display_name: String,
    locale: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireShellTheme {
    id: String,
    stylesheet_url: String,
    window_chrome: String,
    shell_layout: String,
    launcher_layout: String,
    tokens: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireShellSystemStatus {
    protection: String,
    installed_app_count: u32,
    observed_at_unix_ms: u64,
    last_activity_at_unix_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireExtensionContribution {
    Command {
        id: String,
        title: String,
        capability: String,
    },
    SearchProvider {
        id: String,
        title: String,
        capability: String,
    },
    Widget {
        id: String,
        title: String,
        slot: String,
        entrypoint: String,
    },
}

impl WindowChromeVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Compact => "compact",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

impl SystemProtectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Attention => "attention",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "attention" => Some(Self::Attention),
            _ => None,
        }
    }
}

impl ShellSystemStatus {
    pub fn new(
        protection: SystemProtectionStatus,
        installed_app_count: u32,
        observed_at_unix_ms: u64,
        last_activity_at_unix_ms: Option<u64>,
    ) -> Result<Self, ShellSnapshotError> {
        if observed_at_unix_ms == 0
            || observed_at_unix_ms > MAX_JAVASCRIPT_SAFE_INTEGER
            || last_activity_at_unix_ms.is_some_and(|value| {
                value > observed_at_unix_ms || value > MAX_JAVASCRIPT_SAFE_INTEGER
            })
        {
            return Err(ShellSnapshotError::InvalidSystemStatus);
        }
        Ok(Self {
            protection,
            installed_app_count,
            observed_at_unix_ms,
            last_activity_at_unix_ms,
        })
    }

    pub fn protection(&self) -> SystemProtectionStatus {
        self.protection
    }

    pub fn installed_app_count(&self) -> u32 {
        self.installed_app_count
    }

    pub fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    pub fn last_activity_at_unix_ms(&self) -> Option<u64> {
        self.last_activity_at_unix_ms
    }
}

impl ShellApp {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        launchable: bool,
    ) -> Result<Self, ShellSnapshotError> {
        let id = id.into();
        let title = title.into();
        if !valid_namespaced_id(&id) {
            return Err(ShellSnapshotError::InvalidAppId);
        }
        if title.trim().is_empty()
            || title.chars().count() > 256
            || title.chars().any(char::is_control)
        {
            return Err(ShellSnapshotError::InvalidAppTitle);
        }
        Ok(Self {
            id,
            title,
            launchable,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn launchable(&self) -> bool {
        self.launchable
    }
}

impl ShellUser {
    pub fn new(
        display_name: impl Into<String>,
        locale: impl Into<String>,
    ) -> Result<Self, ShellSnapshotError> {
        let display_name = display_name.into();
        let locale = locale.into();
        if display_name.trim().is_empty()
            || display_name.chars().count() > MAX_DISPLAY_NAME_CHARS
            || display_name.chars().any(char::is_control)
        {
            return Err(ShellSnapshotError::InvalidDisplayName);
        }
        if !valid_locale(&locale) {
            return Err(ShellSnapshotError::InvalidLocale);
        }
        Ok(Self {
            display_name,
            locale,
        })
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

impl ShellTheme {
    /// Resolves a server-side theme into the snapshot payload. The compiler and
    /// the client agree on token ids; the `default` wallpaper sentinel is omitted
    /// so the shell can substitute its bundled asset.
    pub fn from_theme(
        stylesheet_url: impl Into<String>,
        theme: &ResolvedTheme,
    ) -> Result<Self, ShellSnapshotError> {
        let tokens = theme
            .tokens()
            .iter()
            .filter_map(|(token, value)| {
                let value = value.as_css_value();
                (value != "default").then(|| (token.id().to_owned(), value))
            })
            .collect();
        Self::from_parts(
            theme.id().to_owned(),
            stylesheet_url.into(),
            theme.window_chrome(),
            theme.shell_layout(),
            theme.launcher_layout(),
            tokens,
        )
    }

    fn from_wire(wire: WireShellTheme) -> Result<Self, ShellSnapshotError> {
        let window_chrome = WindowChromeVariant::parse(&wire.window_chrome)
            .ok_or(ShellSnapshotError::InvalidJson)?;
        let shell_layout =
            ShellLayoutVariant::parse(&wire.shell_layout).ok_or(ShellSnapshotError::InvalidJson)?;
        let launcher_layout = LauncherLayoutVariant::parse(&wire.launcher_layout)
            .ok_or(ShellSnapshotError::InvalidJson)?;
        if wire.tokens.len() > 48 {
            return Err(ShellSnapshotError::InvalidThemeTokens);
        }
        let mut tokens = BTreeMap::new();
        for (id, value) in wire.tokens {
            if !is_known_token_id(&id) || !valid_token_value(&value) {
                return Err(ShellSnapshotError::InvalidThemeTokens);
            }
            tokens.insert(id, value);
        }
        Self::from_parts(
            wire.id,
            wire.stylesheet_url,
            window_chrome,
            shell_layout,
            launcher_layout,
            tokens,
        )
    }

    fn from_parts(
        id: String,
        stylesheet_url: String,
        window_chrome: WindowChromeVariant,
        shell_layout: ShellLayoutVariant,
        launcher_layout: LauncherLayoutVariant,
        tokens: BTreeMap<String, String>,
    ) -> Result<Self, ShellSnapshotError> {
        if !valid_namespaced_id(&id) {
            return Err(ShellSnapshotError::InvalidThemeId);
        }
        if !valid_stylesheet_url(&stylesheet_url) {
            return Err(ShellSnapshotError::InvalidStylesheetUrl);
        }
        Ok(Self {
            id,
            stylesheet_url,
            window_chrome,
            shell_layout,
            launcher_layout,
            tokens,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stylesheet_url(&self) -> &str {
        &self.stylesheet_url
    }

    pub fn window_chrome(&self) -> WindowChromeVariant {
        self.window_chrome
    }

    pub fn shell_layout(&self) -> ShellLayoutVariant {
        self.shell_layout
    }

    pub fn launcher_layout(&self) -> LauncherLayoutVariant {
        self.launcher_layout
    }

    pub fn tokens(&self) -> &BTreeMap<String, String> {
        &self.tokens
    }
}

impl ExtensionContribution {
    pub fn command(
        id: impl Into<String>,
        title: impl Into<String>,
        capability: impl Into<String>,
    ) -> Result<Self, ShellSnapshotError> {
        let id = id.into();
        let title = title.into();
        let capability = capability.into();
        validate_contribution(&id, &title)?;
        if !valid_namespaced_id(&capability) {
            return Err(ShellSnapshotError::InvalidCapability);
        }
        Ok(Self::Command {
            id,
            title,
            capability,
        })
    }

    pub fn search_provider(
        id: impl Into<String>,
        title: impl Into<String>,
        capability: impl Into<String>,
    ) -> Result<Self, ShellSnapshotError> {
        let id = id.into();
        let title = title.into();
        let capability = capability.into();
        validate_contribution(&id, &title)?;
        if !valid_namespaced_id(&capability) {
            return Err(ShellSnapshotError::InvalidCapability);
        }
        Ok(Self::SearchProvider {
            id,
            title,
            capability,
        })
    }

    pub fn widget(
        id: impl Into<String>,
        title: impl Into<String>,
        entrypoint: impl Into<String>,
    ) -> Result<Self, ShellSnapshotError> {
        let id = id.into();
        let title = title.into();
        let entrypoint = entrypoint.into();
        validate_contribution(&id, &title)?;
        if !valid_simple_id(&entrypoint) {
            return Err(ShellSnapshotError::InvalidEntrypoint);
        }
        Ok(Self::Widget {
            id,
            title,
            slot: ExtensionSlot::DashboardWidgets,
            entrypoint,
        })
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Command { id, .. }
            | Self::SearchProvider { id, .. }
            | Self::Widget { id, .. } => id,
        }
    }
}

impl ShellSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shell_build_id: impl Into<String>,
        revision: impl Into<String>,
        user: ShellUser,
        theme: ShellTheme,
        apps: Vec<ShellApp>,
        workspace: Option<String>,
        system_status: ShellSystemStatus,
        contributions: Vec<ExtensionContribution>,
    ) -> Result<Self, ShellSnapshotError> {
        let shell_build_id = shell_build_id.into();
        let revision = revision.into();
        if !valid_opaque_id(&shell_build_id, MAX_BUILD_ID_BYTES) {
            return Err(ShellSnapshotError::InvalidBuildId);
        }
        if !valid_opaque_id(&revision, MAX_REVISION_BYTES) {
            return Err(ShellSnapshotError::InvalidRevision);
        }
        if contributions.len() > MAX_CONTRIBUTIONS {
            return Err(ShellSnapshotError::TooManyContributions);
        }
        let mut ids = contributions
            .iter()
            .map(ExtensionContribution::id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ShellSnapshotError::DuplicateContribution);
        }
        if apps.len() > MAX_APPS {
            return Err(ShellSnapshotError::TooManyApps);
        }
        let mut app_ids = apps.iter().map(ShellApp::id).collect::<Vec<_>>();
        app_ids.sort_unstable();
        if app_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ShellSnapshotError::DuplicateApp);
        }
        if let Some(workspace) = &workspace
            && (workspace.len() > MAX_WORKSPACE_BYTES
                || !matches!(
                    serde_json::from_str::<serde_json::Value>(workspace),
                    Ok(serde_json::Value::Object(_))
                ))
        {
            return Err(ShellSnapshotError::InvalidWorkspace);
        }
        let snapshot = Self {
            snapshot_version: SNAPSHOT_VERSION,
            ui_contract_version: UI_CONTRACT_VERSION,
            extension_api_version: EXTENSION_API_VERSION,
            shell_build_id,
            revision,
            user,
            theme,
            apps,
            workspace,
            system_status,
            mode: ShellMode::Desktop,
            contributions,
        };
        if serde_json::to_vec(&snapshot)
            .map_err(|_| ShellSnapshotError::Serialize)?
            .len()
            > MAX_SNAPSHOT_BYTES
        {
            return Err(ShellSnapshotError::TooLarge);
        }
        Ok(snapshot)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ShellSnapshotError> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(ShellSnapshotError::TooLarge);
        }
        let wire: WireShellSnapshot =
            serde_json::from_slice(bytes).map_err(|_| ShellSnapshotError::InvalidJson)?;
        if wire.snapshot_version != SNAPSHOT_VERSION {
            return Err(ShellSnapshotError::UnsupportedSnapshotVersion(
                wire.snapshot_version,
            ));
        }
        if wire.ui_contract_version != UI_CONTRACT_VERSION {
            return Err(ShellSnapshotError::UnsupportedUiContractVersion(
                wire.ui_contract_version,
            ));
        }
        if wire.extension_api_version != EXTENSION_API_VERSION {
            return Err(ShellSnapshotError::UnsupportedExtensionApiVersion(
                wire.extension_api_version,
            ));
        }
        let user = ShellUser::new(wire.user.display_name, wire.user.locale)?;
        let theme = ShellTheme::from_wire(wire.theme)?;
        let apps = wire
            .apps
            .into_iter()
            .map(|app| ShellApp::new(app.id, app.title, app.launchable))
            .collect::<Result<Vec<_>, _>>()?;
        let workspace = wire.workspace.filter(|value| !value.is_empty());
        let protection = SystemProtectionStatus::parse(&wire.system_status.protection)
            .ok_or(ShellSnapshotError::InvalidSystemStatus)?;
        let system_status = ShellSystemStatus::new(
            protection,
            wire.system_status.installed_app_count,
            wire.system_status.observed_at_unix_ms,
            wire.system_status.last_activity_at_unix_ms,
        )?;
        let contributions = wire
            .contributions
            .into_iter()
            .map(|contribution| match contribution {
                WireExtensionContribution::Command {
                    id,
                    title,
                    capability,
                } => Self::map_command(id, title, capability),
                WireExtensionContribution::SearchProvider {
                    id,
                    title,
                    capability,
                } => ExtensionContribution::search_provider(id, title, capability),
                WireExtensionContribution::Widget {
                    id,
                    title,
                    slot,
                    entrypoint,
                } => {
                    if slot != "dashboard_widgets" {
                        return Err(ShellSnapshotError::InvalidJson);
                    }
                    ExtensionContribution::widget(id, title, entrypoint)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = Self::new(
            wire.shell_build_id,
            wire.revision,
            user,
            theme,
            apps,
            workspace,
            system_status,
            contributions,
        )?;
        let mode = ShellMode::parse(&wire.mode).ok_or(ShellSnapshotError::InvalidJson)?;
        Ok(snapshot.with_mode(mode))
    }

    /// Sets the effective presentation mode resolved for the account/device.
    pub fn with_mode(mut self, mode: ShellMode) -> Self {
        self.mode = mode;
        self
    }

    fn map_command(
        id: String,
        title: String,
        capability: String,
    ) -> Result<ExtensionContribution, ShellSnapshotError> {
        ExtensionContribution::command(id, title, capability)
    }

    pub fn to_json(&self) -> Result<String, ShellSnapshotError> {
        serde_json::to_string(self).map_err(|_| ShellSnapshotError::Serialize)
    }

    pub fn shell_build_id(&self) -> &str {
        &self.shell_build_id
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn user(&self) -> &ShellUser {
        &self.user
    }

    pub fn theme(&self) -> &ShellTheme {
        &self.theme
    }

    pub fn apps(&self) -> &[ShellApp] {
        &self.apps
    }

    pub fn workspace(&self) -> Option<&str> {
        self.workspace.as_deref()
    }

    pub fn mode(&self) -> ShellMode {
        self.mode
    }

    pub fn system_status(&self) -> &ShellSystemStatus {
        &self.system_status
    }

    pub fn contributions(&self) -> &[ExtensionContribution] {
        &self.contributions
    }
}

fn validate_contribution(id: &str, title: &str) -> Result<(), ShellSnapshotError> {
    if !valid_namespaced_id(id) {
        return Err(ShellSnapshotError::InvalidContributionId);
    }
    if title.trim().is_empty() || title.chars().count() > 128 || title.chars().any(char::is_control)
    {
        return Err(ShellSnapshotError::InvalidContributionTitle);
    }
    Ok(())
}

fn valid_stylesheet_url(value: &str) -> bool {
    let Some(hash_and_suffix) = value.strip_prefix("/shell/themes/sha256-") else {
        return false;
    };
    let Some(hash) = hash_and_suffix.strip_suffix(".css") else {
        return false;
    };
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_locale(value: &str) -> bool {
    let mut parts = value.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    if language.len() < 2
        || language.len() > 3
        || !language.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return false;
    }
    parts.all(|part| {
        (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_uppercase()))
            || (part.len() >= 4
                && part.len() <= 8
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    })
}

fn valid_opaque_id(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_namespaced_id(value: &str) -> bool {
    value.len() <= 192 && value.split('.').count() >= 3 && value.split('.').all(valid_simple_id)
}

fn valid_simple_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.ends_with('-')
}

impl fmt::Display for ShellSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "shell snapshot exceeds the size limit"),
            Self::InvalidJson => write!(f, "shell snapshot is invalid"),
            Self::UnsupportedSnapshotVersion(_) => {
                write!(f, "shell snapshot version is unsupported")
            }
            Self::UnsupportedUiContractVersion(_) => {
                write!(f, "shell UI contract version is unsupported")
            }
            Self::UnsupportedExtensionApiVersion(_) => {
                write!(f, "shell extension API version is unsupported")
            }
            Self::InvalidBuildId => write!(f, "shell build ID is invalid"),
            Self::InvalidRevision => write!(f, "shell revision is invalid"),
            Self::InvalidDisplayName => write!(f, "shell user display name is invalid"),
            Self::InvalidLocale => write!(f, "shell user locale is invalid"),
            Self::InvalidThemeId => write!(f, "shell theme id is invalid"),
            Self::InvalidStylesheetUrl => write!(f, "shell theme stylesheet URL is invalid"),
            Self::InvalidThemeTokens => write!(f, "shell theme tokens are invalid"),
            Self::InvalidSystemStatus => write!(f, "shell system status is invalid"),
            Self::InvalidAppId => write!(f, "shell app id is invalid"),
            Self::InvalidAppTitle => write!(f, "shell app title is invalid"),
            Self::TooManyApps => write!(f, "shell has too many apps"),
            Self::DuplicateApp => write!(f, "shell app id is duplicated"),
            Self::InvalidWorkspace => write!(f, "shell workspace is invalid"),
            Self::TooManyContributions => write!(f, "shell has too many contributions"),
            Self::InvalidContributionId => write!(f, "shell contribution ID is invalid"),
            Self::InvalidContributionTitle => write!(f, "shell contribution title is invalid"),
            Self::InvalidCapability => write!(f, "shell contribution capability is invalid"),
            Self::InvalidEntrypoint => write!(f, "shell widget entrypoint is invalid"),
            Self::DuplicateContribution => write!(f, "shell contribution ID is duplicated"),
            Self::Serialize => write!(f, "shell snapshot serialization failed"),
        }
    }
}

impl Error for ShellSnapshotError {}
