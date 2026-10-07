//! Operating-system exposure modes.
//!
//! rumahl OS guides people who are not familiar with PCs or Linux. The default
//! [`OsMode::Guided`] keeps the system closed and lets the OS handle everything,
//! like a phone or a modern desktop. [`OsMode::Advanced`] unlocks most of the
//! system for experienced users, and [`OsMode::Developer`] opens everything and
//! enables the terminal, SSH and the web console.
//!
//! The mode is a *policy* input, not an identity: it decides what the shell,
//! file explorer and apps may see and which services are available. Rust remains
//! authoritative for permissions; the mode never grants a capability by itself.

use std::error::Error;
use std::fmt;

/// How much of the system is exposed to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsMode {
    /// Closed, guided default for everyone.
    #[default]
    Guided,
    /// Most of the system unlocked, without developer tools.
    Advanced,
    /// Fully unlocked, including terminal, SSH and the web console.
    Developer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsModeError {
    Unknown,
}

/// The capabilities a given [`OsMode`] exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsModePolicy {
    mode: OsMode,
}

impl OsMode {
    pub fn parse(value: &str) -> Result<Self, OsModeError> {
        match value {
            "guided" => Ok(Self::Guided),
            "advanced" => Ok(Self::Advanced),
            "developer" => Ok(Self::Developer),
            _ => Err(OsModeError::Unknown),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Guided => "guided",
            Self::Advanced => "advanced",
            Self::Developer => "developer",
        }
    }

    pub fn policy(&self) -> OsModePolicy {
        OsModePolicy { mode: *self }
    }
}

impl OsModePolicy {
    pub fn mode(&self) -> OsMode {
        self.mode
    }

    /// The guided default, in which the OS is closed.
    pub fn is_guided(&self) -> bool {
        self.mode == OsMode::Guided
    }

    /// Advanced settings are available from [`OsMode::Advanced`] upwards.
    pub fn can_change_advanced_settings(&self) -> bool {
        matches!(self.mode, OsMode::Advanced | OsMode::Developer)
    }

    /// The file explorer may browse system areas from [`OsMode::Advanced`] upwards.
    pub fn can_browse_system_files(&self) -> bool {
        matches!(self.mode, OsMode::Advanced | OsMode::Developer)
    }

    /// The terminal is only available in [`OsMode::Developer`].
    pub fn can_open_terminal(&self) -> bool {
        self.mode == OsMode::Developer
    }

    /// SSH access is only available in [`OsMode::Developer`].
    pub fn can_use_ssh(&self) -> bool {
        self.mode == OsMode::Developer
    }

    /// The browser-based web console is only available in [`OsMode::Developer`].
    pub fn can_use_web_console(&self) -> bool {
        self.mode == OsMode::Developer
    }
}

impl fmt::Display for OsMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for OsModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown rumahl OS mode"),
        }
    }
}

impl Error for OsModeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_modes() {
        assert_eq!(OsMode::parse("guided").unwrap(), OsMode::Guided);
        assert_eq!(OsMode::parse("advanced").unwrap(), OsMode::Advanced);
        assert_eq!(OsMode::parse("developer").unwrap(), OsMode::Developer);
        assert_eq!(OsMode::parse("root").unwrap_err(), OsModeError::Unknown);
        assert_eq!(OsMode::Developer.to_string(), "developer");
    }

    #[test]
    fn defaults_to_guided() {
        assert_eq!(OsMode::default(), OsMode::Guided);
    }

    #[test]
    fn guided_is_closed() {
        let policy = OsMode::Guided.policy();
        assert!(policy.is_guided());
        assert!(!policy.can_change_advanced_settings());
        assert!(!policy.can_browse_system_files());
        assert!(!policy.can_open_terminal());
        assert!(!policy.can_use_ssh());
        assert!(!policy.can_use_web_console());
    }

    #[test]
    fn advanced_unlocks_the_system_but_not_developer_tools() {
        let policy = OsMode::Advanced.policy();
        assert!(policy.can_change_advanced_settings());
        assert!(policy.can_browse_system_files());
        assert!(!policy.can_open_terminal());
        assert!(!policy.can_use_ssh());
        assert!(!policy.can_use_web_console());
    }

    #[test]
    fn developer_unlocks_everything() {
        let policy = OsMode::Developer.policy();
        assert!(policy.can_change_advanced_settings());
        assert!(policy.can_browse_system_files());
        assert!(policy.can_open_terminal());
        assert!(policy.can_use_ssh());
        assert!(policy.can_use_web_console());
    }
}
