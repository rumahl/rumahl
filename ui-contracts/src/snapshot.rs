use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{EXTENSION_API_VERSION, SNAPSHOT_VERSION, UI_CONTRACT_VERSION};

const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
const MAX_BUILD_ID_BYTES: usize = 128;
const MAX_REVISION_BYTES: usize = 128;
const MAX_DISPLAY_NAME_CHARS: usize = 128;
const MAX_CONTRIBUTIONS: usize = 512;
const MAX_JAVASCRIPT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowChromeVariant {
    Standard,
    Compact,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellTheme {
    stylesheet_url: String,
    window_chrome: WindowChromeVariant,
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
    system_status: ShellSystemStatus,
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
    InvalidStylesheetUrl,
    InvalidSystemStatus,
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
    system_status: WireShellSystemStatus,
    contributions: Vec<WireExtensionContribution>,
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
    stylesheet_url: String,
    window_chrome: String,
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
    pub fn new(
        stylesheet_url: impl Into<String>,
        window_chrome: WindowChromeVariant,
    ) -> Result<Self, ShellSnapshotError> {
        let stylesheet_url = stylesheet_url.into();
        if !valid_stylesheet_url(&stylesheet_url) {
            return Err(ShellSnapshotError::InvalidStylesheetUrl);
        }
        Ok(Self {
            stylesheet_url,
            window_chrome,
        })
    }

    pub fn stylesheet_url(&self) -> &str {
        &self.stylesheet_url
    }

    pub fn window_chrome(&self) -> WindowChromeVariant {
        self.window_chrome
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
    pub fn new(
        shell_build_id: impl Into<String>,
        revision: impl Into<String>,
        user: ShellUser,
        theme: ShellTheme,
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
        Ok(Self {
            snapshot_version: SNAPSHOT_VERSION,
            ui_contract_version: UI_CONTRACT_VERSION,
            extension_api_version: EXTENSION_API_VERSION,
            shell_build_id,
            revision,
            user,
            theme,
            system_status,
            contributions,
        })
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
        let window_chrome = WindowChromeVariant::parse(&wire.theme.window_chrome)
            .ok_or(ShellSnapshotError::InvalidJson)?;
        let theme = ShellTheme::new(wire.theme.stylesheet_url, window_chrome)?;
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
        Self::new(
            wire.shell_build_id,
            wire.revision,
            user,
            theme,
            system_status,
            contributions,
        )
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
            Self::InvalidStylesheetUrl => write!(f, "shell theme stylesheet URL is invalid"),
            Self::InvalidSystemStatus => write!(f, "shell system status is invalid"),
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
