use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceKind(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceKindError {
    Empty,
    TooLong,
    InvalidStart(char),
    InvalidCharacter(char),
    InvalidEnd,
}

impl ResourceKind {
    pub const MAX_LENGTH: usize = 64;

    pub fn parse(value: impl Into<String>) -> Result<Self, ResourceKindError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ResourceKindError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(ResourceKindError::TooLong);
        }

        let mut characters = value.chars();

        let first = characters
            .next()
            .expect("non-empty resource kind must have a first character");

        if !first.is_ascii_lowercase() {
            return Err(ResourceKindError::InvalidStart(first));
        }

        for character in characters {
            if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
                return Err(ResourceKindError::InvalidCharacter(character));
            }
        }

        if value.ends_with('-') {
            return Err(ResourceKindError::InvalidEnd);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for ResourceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "resource kind must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "resource kind must not exceed {} characters",
                    ResourceKind::MAX_LENGTH
                )
            }

            Self::InvalidStart(character) => {
                write!(
                    f,
                    "resource kind must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in resource kind")
            }

            Self::InvalidEnd => {
                write!(f, "resource kind must not end with '-'")
            }
        }
    }
}

impl Error for ResourceKindError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_file_kind() {
        let kind = ResourceKind::parse("file").unwrap();

        assert_eq!(kind.as_str(), "file");
    }

    #[test]
    fn accepts_hyphenated_kind() {
        assert!(ResourceKind::parse("network-interface").is_ok());
    }

    #[test]
    fn rejects_empty_kind() {
        assert_eq!(
            ResourceKind::parse("").unwrap_err(),
            ResourceKindError::Empty
        );
    }

    #[test]
    fn rejects_uppercase_kind() {
        assert_eq!(
            ResourceKind::parse("File").unwrap_err(),
            ResourceKindError::InvalidStart('F')
        );
    }

    #[test]
    fn rejects_invalid_character() {
        assert_eq!(
            ResourceKind::parse("file_kind").unwrap_err(),
            ResourceKindError::InvalidCharacter('_')
        );
    }

    #[test]
    fn rejects_trailing_hyphen() {
        assert_eq!(
            ResourceKind::parse("file-").unwrap_err(),
            ResourceKindError::InvalidEnd
        );
    }
}
