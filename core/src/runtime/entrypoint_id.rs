use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeEntrypointId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEntrypointIdError {
    Empty,
    TooLong,
    InvalidStart(char),
    InvalidCharacter(char),
    InvalidEnd,
}

impl RuntimeEntrypointId {
    pub const MAX_LENGTH: usize = 63;

    pub fn parse(value: impl Into<String>) -> Result<Self, RuntimeEntrypointIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(RuntimeEntrypointIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(RuntimeEntrypointIdError::TooLong);
        }

        let mut characters = value.chars();

        let first = characters
            .next()
            .expect("non-empty runtime entrypoint id must have a first character");

        if !first.is_ascii_lowercase() {
            return Err(RuntimeEntrypointIdError::InvalidStart(first));
        }

        for character in characters {
            if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
                return Err(RuntimeEntrypointIdError::InvalidCharacter(character));
            }
        }

        if value.ends_with('-') {
            return Err(RuntimeEntrypointIdError::InvalidEnd);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeEntrypointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RuntimeEntrypointId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for RuntimeEntrypointId {
    type Error = RuntimeEntrypointIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RuntimeEntrypointId {
    type Error = RuntimeEntrypointIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for RuntimeEntrypointIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "runtime entrypoint id must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "runtime entrypoint id must not exceed {} bytes",
                    RuntimeEntrypointId::MAX_LENGTH
                )
            }

            Self::InvalidStart(character) => {
                write!(
                    f,
                    "runtime entrypoint id must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "runtime entrypoint id contains invalid character '{character}'"
                )
            }

            Self::InvalidEnd => {
                write!(f, "runtime entrypoint id must not end with '-'")
            }
        }
    }
}

impl Error for RuntimeEntrypointIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_manifest_entrypoint_ids() {
        assert_eq!(RuntimeEntrypointId::parse("main").unwrap().as_str(), "main");

        assert!(RuntimeEntrypointId::parse("service").is_ok());
        assert!(RuntimeEntrypointId::parse("widget-recent").is_ok());
    }

    #[test]
    fn rejects_invalid_entrypoint_ids() {
        assert_eq!(
            RuntimeEntrypointId::parse("").unwrap_err(),
            RuntimeEntrypointIdError::Empty
        );

        assert_eq!(
            RuntimeEntrypointId::parse("Main").unwrap_err(),
            RuntimeEntrypointIdError::InvalidStart('M')
        );

        assert_eq!(
            RuntimeEntrypointId::parse("main.path").unwrap_err(),
            RuntimeEntrypointIdError::InvalidCharacter('.')
        );

        assert_eq!(
            RuntimeEntrypointId::parse("main-").unwrap_err(),
            RuntimeEntrypointIdError::InvalidEnd
        );
    }
}
