use std::error::Error;
use std::fmt;

use crate::RuntimeEntrypointId;

/// Declares that an app ships a connector bundle for an external service.
///
/// The OS installs the bundle into the target (for example a Nextcloud app or a
/// Plex plug-in), registers the app as an OIDC relying party for it, and lets it
/// reach the OS over the persistent channel. The bundle is a container-artifact
/// entrypoint of the same app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorDeclaration {
    target: ConnectorTarget,
    entrypoint: RuntimeEntrypointId,
}

impl ConnectorDeclaration {
    pub fn new(target: ConnectorTarget, entrypoint: RuntimeEntrypointId) -> Self {
        Self { target, entrypoint }
    }

    pub fn target(&self) -> &ConnectorTarget {
        &self.target
    }

    pub fn entrypoint(&self) -> &RuntimeEntrypointId {
        &self.entrypoint
    }
}

/// A validated identifier of the external service a connector targets
/// (for example `nextcloud` or `plex`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorTarget(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorTargetError {
    Empty,
    TooLong,
    InvalidCharacter,
}

impl ConnectorTarget {
    pub const MAX_LENGTH: usize = 64;

    pub fn parse(value: impl Into<String>) -> Result<Self, ConnectorTargetError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ConnectorTargetError::Empty);
        }
        if value.len() > Self::MAX_LENGTH {
            return Err(ConnectorTargetError::TooLong);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.')
        {
            return Err(ConnectorTargetError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectorTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ConnectorTargetError {}

impl fmt::Display for ConnectorTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("connector target is empty"),
            Self::TooLong => write!(
                f,
                "connector target exceeds {} characters",
                ConnectorTarget::MAX_LENGTH
            ),
            Self::InvalidCharacter => f.write_str("connector target has an invalid character"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_targets_and_rejects_bad_ones() {
        assert_eq!(ConnectorTarget::parse("nextcloud").unwrap().as_str(), "nextcloud");
        assert_eq!(ConnectorTarget::parse("plex").unwrap().as_str(), "plex");
        assert_eq!(
            ConnectorTarget::parse("next-cloud.2").unwrap().as_str(),
            "next-cloud.2"
        );
        assert!(matches!(ConnectorTarget::parse(""), Err(ConnectorTargetError::Empty)));
        assert!(matches!(
            ConnectorTarget::parse("Nextcloud"),
            Err(ConnectorTargetError::InvalidCharacter)
        ));
        assert!(matches!(
            ConnectorTarget::parse("a".repeat(65)),
            Err(ConnectorTargetError::TooLong)
        ));
    }
}
