use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl PermissionId {
    pub const MAX_LENGTH: usize = 255;
    pub const MIN_SEGMENTS: usize = 3;

    pub fn parse(value: impl Into<String>) -> Result<Self, PermissionIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(PermissionIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(PermissionIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < Self::MIN_SEGMENTS {
            return Err(PermissionIdError::TooFewSegments);
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

fn validate_segment(segment: &str) -> Result<(), PermissionIdError> {
    if segment.is_empty() {
        return Err(PermissionIdError::EmptySegment);
    }

    let mut characters = segment.chars();

    let first = characters
        .next()
        .expect("non-empty permission segment must have a first character");

    if !first.is_ascii_lowercase() {
        return Err(PermissionIdError::InvalidSegmentStart(first));
    }

    for character in characters {
        if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
            return Err(PermissionIdError::InvalidCharacter(character));
        }
    }

    if segment.ends_with('-') {
        return Err(PermissionIdError::InvalidSegmentEnd);
    }

    Ok(())
}

impl fmt::Display for PermissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for PermissionIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "permission ID must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "permission ID must not exceed {} characters",
                    PermissionId::MAX_LENGTH
                )
            }

            Self::TooFewSegments => {
                write!(
                    f,
                    "permission ID must contain at least {} segments",
                    PermissionId::MIN_SEGMENTS
                )
            }

            Self::EmptySegment => {
                write!(f, "permission ID must not contain empty segments")
            }

            Self::InvalidSegmentStart(character) => {
                write!(
                    f,
                    "permission ID segment must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in permission ID")
            }

            Self::InvalidSegmentEnd => {
                write!(f, "permission ID segment must not end with '-'")
            }
        }
    }
}

impl Error for PermissionIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_rumahl_permission() {
        let permission = PermissionId::parse("rumahl.files.read").unwrap();

        assert_eq!(permission.as_str(), "rumahl.files.read");
    }

    #[test]
    fn accepts_third_party_permission() {
        assert!(PermissionId::parse("com.rumahl.notes.export").is_ok());
    }

    #[test]
    fn rejects_too_few_segments() {
        assert_eq!(
            PermissionId::parse("files.read").unwrap_err(),
            PermissionIdError::TooFewSegments
        );
    }

    #[test]
    fn rejects_uppercase() {
        assert_eq!(
            PermissionId::parse("rumahl.files.Read").unwrap_err(),
            PermissionIdError::InvalidSegmentStart('R')
        );
    }
}
