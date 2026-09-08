use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceNamespace(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceNamespaceError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl ResourceNamespace {
    pub const MAX_LENGTH: usize = 255;
    pub const MIN_SEGMENTS: usize = 2;

    pub fn parse(value: impl Into<String>) -> Result<Self, ResourceNamespaceError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ResourceNamespaceError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(ResourceNamespaceError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < Self::MIN_SEGMENTS {
            return Err(ResourceNamespaceError::TooFewSegments);
        }

        for segment in segments {
            validate_segment(segment)?;
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_segment(segment: &str) -> Result<(), ResourceNamespaceError> {
    if segment.is_empty() {
        return Err(ResourceNamespaceError::EmptySegment);
    }

    let mut characters = segment.chars();

    let first = characters
        .next()
        .expect("non-empty namespace segment must contain a first character");

    if !first.is_ascii_lowercase() {
        return Err(ResourceNamespaceError::InvalidSegmentStart(first));
    }

    for character in characters {
        if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
            return Err(ResourceNamespaceError::InvalidCharacter(character));
        }
    }

    if segment.ends_with('-') {
        return Err(ResourceNamespaceError::InvalidSegmentEnd);
    }

    Ok(())
}

impl fmt::Display for ResourceNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for ResourceNamespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "resource namespace must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "resource namespace must not exceed {} characters",
                    ResourceNamespace::MAX_LENGTH
                )
            }

            Self::TooFewSegments => {
                write!(
                    f,
                    "resource namespace must contain at least {} segments",
                    ResourceNamespace::MIN_SEGMENTS
                )
            }

            Self::EmptySegment => {
                write!(f, "resource namespace must not contain empty segments")
            }

            Self::InvalidSegmentStart(character) => {
                write!(
                    f,
                    "resource namespace segment must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in resource namespace")
            }

            Self::InvalidSegmentEnd => {
                write!(f, "resource namespace segment must not end with '-'")
            }
        }
    }
}

impl Error for ResourceNamespaceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_rumahl_namespace() {
        let namespace = ResourceNamespace::parse("rumahl.files").unwrap();

        assert_eq!(namespace.as_str(), "rumahl.files");
    }

    #[test]
    fn accepts_third_party_namespace() {
        assert!(ResourceNamespace::parse("com.rumahl.notes").is_ok());
    }

    #[test]
    fn rejects_single_segment() {
        assert_eq!(
            ResourceNamespace::parse("files").unwrap_err(),
            ResourceNamespaceError::TooFewSegments
        );
    }

    #[test]
    fn rejects_uppercase_namespace() {
        assert_eq!(
            ResourceNamespace::parse("rumahl.Files").unwrap_err(),
            ResourceNamespaceError::InvalidSegmentStart('F')
        );
    }
}
