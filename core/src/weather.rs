//! Severe-weather warnings owned by the operating system.
//!
//! Weather-warning apps are *sources*: they register with the OS and publish
//! bounded warning records. The OS owns validation, expiry, the active-warning
//! cache, the warning log and the native presentation. An app never renders a
//! warning or keeps it alive on its own.
//!
//! The data model follows the prototype's NINA widget (id, headline,
//! description, severity, level, region, event, sender, effective, expires,
//! area) and its `critical` / `emergency` / `warning` / `info` level names.

use crate::AppId;
use std::error::Error;
use std::fmt;

/// Manifest permission an app declares before it may register as a warning source.
pub const WEATHER_WARNING_CAPABILITY: &str = "com.rumahl.os.warnings";

/// Built-in OS source used for native warnings and test warnings.
pub const OS_WARNING_SOURCE_ID: &str = "com.rumahl.os";

pub const MAX_ACTIVE_WARNINGS: usize = 64;
pub const MAX_WARNINGS_PER_SOURCE: usize = 16;
pub const MAX_WARNING_ID_BYTES: usize = 64;
pub const MAX_WARNING_HEADLINE_BYTES: usize = 160;
pub const MAX_WARNING_DESCRIPTION_BYTES: usize = 2048;
pub const MAX_WARNING_REGION_BYTES: usize = 120;
pub const MAX_WARNING_EVENT_BYTES: usize = 120;
pub const MAX_WARNING_SEVERITY_BYTES: usize = 64;
pub const MAX_WARNING_SENDER_BYTES: usize = 120;
pub const MAX_WARNING_INSTRUCTION_BYTES: usize = 1024;
pub const MAX_WARNING_AREA_BYTES: usize = 512;
pub const MAX_WARNING_SOURCE_NAME_BYTES: usize = 200;

/// How urgent a warning is. The order is significant and drives the banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningLevel {
    Info,
    Warning,
    Critical,
    Emergency,
}

impl WarningLevel {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "critical" => Some(Self::Critical),
            "emergency" => Some(Self::Emergency),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Emergency => "emergency",
        }
    }

    /// Higher means more urgent.
    pub fn rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Critical => 2,
            Self::Emergency => 3,
        }
    }
}

impl fmt::Display for WarningLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherWarningError {
    /// A field is missing, too long, malformed or outside its allowed range.
    Invalid,
    /// The publishing source has not registered with the OS.
    UnknownSource,
    /// The active-warning cache is full.
    Limit,
    /// Storage is unavailable.
    Unavailable,
}

impl fmt::Display for WeatherWarningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid weather warning",
            Self::UnknownSource => "unknown weather warning source",
            Self::Limit => "too many active weather warnings",
            Self::Unavailable => "weather warning storage unavailable",
        })
    }
}

impl Error for WeatherWarningError {}

/// A warning app that has registered with the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningSource {
    pub app_id: String,
    pub display_name: String,
    pub registered_at_ms: i64,
    pub updated_at_ms: i64,
}

/// One warning an app wants the OS to present.
///
/// A draft is untrusted input until [`Self::validate`] has accepted it. The
/// repository validates again before writing, so a caller cannot bypass the
/// bounds by constructing the struct directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherWarningDraft {
    pub source_id: String,
    pub id: String,
    pub level: WarningLevel,
    pub headline: String,
    pub description: String,
    pub region: String,
    pub event: String,
    pub severity: String,
    pub sender: String,
    pub instruction: String,
    pub area: String,
    pub effective_at_ms: Option<i64>,
    pub expires_at_ms: i64,
    pub test: bool,
}

impl WeatherWarningDraft {
    /// Validates and normalizes the draft.
    pub fn validate(mut self, now_ms: i64) -> Result<Self, WeatherWarningError> {
        if AppId::parse(&self.source_id).is_err() {
            return Err(WeatherWarningError::Invalid);
        }
        if !valid_warning_id(&self.id) {
            return Err(WeatherWarningError::Invalid);
        }
        self.headline = bounded_text(&self.headline, MAX_WARNING_HEADLINE_BYTES, false)?;
        self.description = bounded_text(&self.description, MAX_WARNING_DESCRIPTION_BYTES, true)?;
        self.region = bounded_text(&self.region, MAX_WARNING_REGION_BYTES, false)?;
        self.event = bounded_text(&self.event, MAX_WARNING_EVENT_BYTES, false)?;
        self.severity = bounded_text(&self.severity, MAX_WARNING_SEVERITY_BYTES, false)?;
        self.sender = bounded_text(&self.sender, MAX_WARNING_SENDER_BYTES, false)?;
        self.instruction = bounded_text(&self.instruction, MAX_WARNING_INSTRUCTION_BYTES, true)?;
        self.area = bounded_text(&self.area, MAX_WARNING_AREA_BYTES, true)?;
        if self.headline.is_empty() {
            return Err(WeatherWarningError::Invalid);
        }
        let max_lifetime_ms = if self.test {
            60 * 60 * 1_000
        } else {
            7 * 24 * 60 * 60 * 1_000
        };
        if self.expires_at_ms <= now_ms || self.expires_at_ms - now_ms > max_lifetime_ms {
            return Err(WeatherWarningError::Invalid);
        }
        if let Some(effective) = self.effective_at_ms
            && (effective < 0 || effective > self.expires_at_ms)
        {
            return Err(WeatherWarningError::Invalid);
        }
        Ok(self)
    }
}

/// A warning the OS has accepted and is presenting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherWarning {
    pub id: String,
    pub source: WarningSource,
    pub level: WarningLevel,
    pub headline: String,
    pub description: String,
    pub region: String,
    pub event: String,
    pub severity: String,
    pub sender: String,
    pub instruction: String,
    pub area: String,
    pub effective_at_ms: Option<i64>,
    pub expires_at_ms: i64,
    pub test: bool,
    pub published_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WeatherWarning {
    pub fn is_active_at(&self, now_ms: i64) -> bool {
        self.expires_at_ms > now_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningLogAction {
    Published,
    Updated,
    Cleared,
    Expired,
}

impl WarningLogAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::Updated => "updated",
            Self::Cleared => "cleared",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "published" => Some(Self::Published),
            "updated" => Some(Self::Updated),
            "cleared" => Some(Self::Cleared),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

/// Audit-friendly active period of one warning. The table is append-only in
/// spirit: an update closes the previous period and starts a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherWarningLogEntry {
    pub sequence: u64,
    pub source_id: String,
    pub warning_id: String,
    pub action: WarningLogAction,
    pub level: WarningLevel,
    pub headline: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub expires_at_ms: i64,
    pub test: bool,
}

/// Persistence boundary for the OS warning service.
pub trait WeatherWarningRepository: Send + Sync + 'static {
    /// Registers or refreshes a warning source. Registration is idempotent.
    fn register_source(
        &self,
        app_id: &str,
        display_name: &str,
        now_ms: i64,
    ) -> Result<WarningSource, WeatherWarningError>;

    fn source(&self, app_id: &str) -> Result<Option<WarningSource>, WeatherWarningError>;

    /// Inserts or updates one warning. The source must be registered first.
    fn publish(
        &self,
        draft: WeatherWarningDraft,
        now_ms: i64,
    ) -> Result<WeatherWarning, WeatherWarningError>;

    /// Clears one warning, a set of warning ids, or every warning of a source
    /// when `ids` is `None`. Returns the number of removed active warnings.
    fn clear(
        &self,
        source_id: &str,
        ids: Option<&[String]>,
        now_ms: i64,
    ) -> Result<u32, WeatherWarningError>;

    /// Active warnings at `now_ms`, most urgent first.
    fn active(&self, now_ms: i64) -> Result<Vec<WeatherWarning>, WeatherWarningError>;

    /// Most recent warning-log periods, newest first.
    fn history(&self, limit: usize) -> Result<Vec<WeatherWarningLogEntry>, WeatherWarningError>;

    /// Clears every warning owned by an app that is being uninstalled.
    fn forget_source(&self, app_id: &str, now_ms: i64) -> Result<u32, WeatherWarningError>;
}

fn valid_warning_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WARNING_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn bounded_text(
    value: &str,
    maximum: usize,
    multiline: bool,
) -> Result<String, WeatherWarningError> {
    if value.len() > maximum
        || value.chars().any(|character| {
            character.is_control() && !(multiline && matches!(character, '\n' | '\t'))
        })
    {
        return Err(WeatherWarningError::Invalid);
    }
    Ok(value.trim().to_owned())
}

pub fn valid_warning_source_name(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_WARNING_SOURCE_NAME_BYTES
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> WeatherWarningDraft {
        WeatherWarningDraft {
            source_id: "com.example.weather".to_owned(),
            id: "warning-1".to_owned(),
            level: WarningLevel::Warning,
            headline: "  Severe storm  ".to_owned(),
            description: "Gusts up to 120 km/h.".to_owned(),
            region: "Berlin".to_owned(),
            event: "STORM".to_owned(),
            severity: "Severe".to_owned(),
            sender: "DWD".to_owned(),
            instruction: "Secure loose objects.".to_owned(),
            area: "Berlin".to_owned(),
            effective_at_ms: None,
            expires_at_ms: 2_000,
            test: false,
        }
    }

    #[test]
    fn levels_order_from_info_to_emergency() {
        assert!(WarningLevel::Emergency > WarningLevel::Critical);
        assert!(WarningLevel::Critical > WarningLevel::Warning);
        assert!(WarningLevel::Warning > WarningLevel::Info);
        assert_eq!(
            WarningLevel::parse("critical"),
            Some(WarningLevel::Critical)
        );
        assert_eq!(WarningLevel::parse("unknown"), None);
        assert_eq!(WarningLevel::Emergency.as_str(), "emergency");
    }

    #[test]
    fn accepts_and_normalizes_a_bounded_warning() {
        let warning = draft().validate(1_000).unwrap();
        assert_eq!(warning.headline, "Severe storm");
        assert_eq!(warning.expires_at_ms, 2_000);
    }

    #[test]
    fn rejects_malformed_ids_text_and_expiry() {
        for mut invalid in [
            WeatherWarningDraft {
                id: "../escape".to_owned(),
                ..draft()
            },
            WeatherWarningDraft {
                source_id: "not-an-app".to_owned(),
                ..draft()
            },
            WeatherWarningDraft {
                headline: "\u{0}".to_owned(),
                ..draft()
            },
            WeatherWarningDraft {
                expires_at_ms: 999,
                ..draft()
            },
            WeatherWarningDraft {
                expires_at_ms: 8 * 24 * 60 * 60 * 1_000,
                ..draft()
            },
            WeatherWarningDraft {
                effective_at_ms: Some(2_500),
                ..draft()
            },
        ] {
            assert_eq!(
                invalid.clone().validate(1_000),
                Err(WeatherWarningError::Invalid),
                "{invalid:?}"
            );
            invalid.test = false;
        }
    }

    #[test]
    fn test_warnings_expire_quickly() {
        let mut short = draft();
        short.test = true;
        short.expires_at_ms = 1_000 + 30 * 60 * 1_000;
        assert!(short.clone().validate(1_000).is_ok());
        short.expires_at_ms = 1_000 + 2 * 60 * 60 * 1_000;
        assert_eq!(short.validate(1_000), Err(WeatherWarningError::Invalid));
    }
}
