use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceKey(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceKeyError {
    Empty,
    TooLong,
    InvalidCharacter(char),
}

impl ResourceKey {
    pub const MAX_LENGTH: usize = 255;

    pub fn parse(value: impl Into<String>) -> Result<Self, ResourceKeyError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ResourceKeyError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(ResourceKeyError::TooLong);
        }

        for character in value.chars() {
            if !character.is_ascii_alphanumeric()
                && character != '-'
                && character != '_'
                && character != '.'
            {
                return Err(ResourceKeyError::InvalidCharacter(character));
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for ResourceKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "resource key must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "resource key must not exceed {} bytes",
                    ResourceKey::MAX_LENGTH
                )
            }

            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in resource key")
            }
        }
    }
}

impl Error for ResourceKeyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_key() {
        let key = ResourceKey::parse("file").unwrap();

        assert_eq!(key.as_str(), "file");
        assert_eq!(key.to_string(), "file");
    }

    #[test]
    fn accepts_valid_characters() {
        assert!(ResourceKey::parse("file.txt").is_ok());
        assert!(ResourceKey::parse("file-name").is_ok());
        assert!(ResourceKey::parse("file_name").is_ok());
        assert!(ResourceKey::parse("File123").is_ok());
        assert!(ResourceKey::parse("foo.bar-baz_123").is_ok());
    }

    #[test]
    fn rejects_empty_key() {
        assert_eq!(ResourceKey::parse("").unwrap_err(), ResourceKeyError::Empty);
    }

    #[test]
    fn rejects_too_long_key() {
        let value = "a".repeat(ResourceKey::MAX_LENGTH + 1);

        assert_eq!(
            ResourceKey::parse(value).unwrap_err(),
            ResourceKeyError::TooLong
        );
    }

    #[test]
    fn rejects_invalid_characters() {
        assert_eq!(
            ResourceKey::parse("hello world").unwrap_err(),
            ResourceKeyError::InvalidCharacter(' ')
        );

        assert_eq!(
            ResourceKey::parse("foo:bar").unwrap_err(),
            ResourceKeyError::InvalidCharacter(':')
        );

        assert_eq!(
            ResourceKey::parse("../etc/passwd").unwrap_err(),
            ResourceKeyError::InvalidCharacter('/')
        );

        assert_eq!(
            ResourceKey::parse("/mnt/data").unwrap_err(),
            ResourceKeyError::InvalidCharacter('/')
        );
    }

    #[test]
    fn rejects_unicode() {
        assert_eq!(
            ResourceKey::parse("möbel").unwrap_err(),
            ResourceKeyError::InvalidCharacter('ö')
        );
    }

    #[test]
    fn supports_equality_and_hashing() {
        use std::collections::HashSet;

        let key1 = ResourceKey::parse("file").unwrap();
        let key2 = ResourceKey::parse("file").unwrap();

        assert_eq!(key1, key2);

        let mut keys = HashSet::new();
        keys.insert(key1);

        assert!(keys.contains(&key2));
    }
}
