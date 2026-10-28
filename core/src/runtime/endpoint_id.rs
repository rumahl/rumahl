use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeEndpointId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEndpointIdError {
    Empty,
    TooLong,
    InvalidStart(char),
    InvalidCharacter(char),
    InvalidEnd,
}

impl RuntimeEndpointId {
    pub const MAX_LENGTH: usize = 63;

    pub fn parse(value: impl Into<String>) -> Result<Self, RuntimeEndpointIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(RuntimeEndpointIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(RuntimeEndpointIdError::TooLong);
        }

        let mut characters = value.chars();

        let first = characters
            .next()
            .expect("non-empty runtime endpoint id must have a first character");

        if !first.is_ascii_lowercase() {
            return Err(RuntimeEndpointIdError::InvalidStart(first));
        }

        for character in characters {
            if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
                return Err(RuntimeEndpointIdError::InvalidCharacter(character));
            }
        }

        if value.ends_with('-') {
            return Err(RuntimeEndpointIdError::InvalidEnd);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeEndpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RuntimeEndpointId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for RuntimeEndpointId {
    type Error = RuntimeEndpointIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RuntimeEndpointId {
    type Error = RuntimeEndpointIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for RuntimeEndpointIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "runtime endpoint id must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "runtime endpoint id must not exceed {} bytes",
                    RuntimeEndpointId::MAX_LENGTH
                )
            }

            Self::InvalidStart(character) => {
                write!(
                    f,
                    "runtime endpoint id must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "runtime endpoint id contains invalid character '{character}'"
                )
            }

            Self::InvalidEnd => {
                write!(f, "runtime endpoint id must not end with '-'")
            }
        }
    }
}

impl Error for RuntimeEndpointIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_manifest_endpoint_id() {
        assert_eq!(RuntimeEndpointId::parse("web").unwrap().as_str(), "web");
    }

    #[test]
    fn rejects_invalid_endpoint_id() {
        assert_eq!(
            RuntimeEndpointId::parse("host:8096").unwrap_err(),
            RuntimeEndpointIdError::InvalidCharacter(':')
        );
    }
}
