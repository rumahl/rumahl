use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl ServiceId {
    pub const MAX_LENGTH: usize = 255;
    pub const MAX_SEGMENT_LENGTH: usize = 63;
    pub const MIN_SEGMENTS: usize = 2;

    pub fn parse(value: impl Into<String>) -> Result<Self, ServiceIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ServiceIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(ServiceIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < Self::MIN_SEGMENTS {
            return Err(ServiceIdError::TooFewSegments);
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

fn validate_segment(segment: &str) -> Result<(), ServiceIdError> {
    if segment.is_empty() {
        return Err(ServiceIdError::EmptySegment);
    }

    if segment.len() > ServiceId::MAX_SEGMENT_LENGTH {
        return Err(ServiceIdError::SegmentTooLong);
    }

    let mut characters = segment.chars();

    let first = characters
        .next()
        .expect("non-empty service ID segment must have a first character");

    if !first.is_ascii_lowercase() {
        return Err(ServiceIdError::InvalidSegmentStart(first));
    }

    for character in characters {
        if !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && character != '-'
        {
            return Err(ServiceIdError::InvalidCharacter(character));
        }
    }

    if segment.ends_with('-') {
        return Err(ServiceIdError::InvalidSegmentEnd);
    }

    Ok(())
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ServiceId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for ServiceId {
    type Error = ServiceIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ServiceId {
    type Error = ServiceIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for ServiceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "service ID must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "service ID must not exceed {} characters",
                    ServiceId::MAX_LENGTH
                )
            }

            Self::TooFewSegments => {
                write!(
                    f,
                    "service ID must contain at least {} segments",
                    ServiceId::MIN_SEGMENTS
                )
            }

            Self::EmptySegment => {
                write!(f, "service ID must not contain empty segments")
            }

            Self::SegmentTooLong => {
                write!(
                    f,
                    "service ID segment must not exceed {} characters",
                    ServiceId::MAX_SEGMENT_LENGTH
                )
            }

            Self::InvalidSegmentStart(character) => {
                write!(
                    f,
                    "service ID segment must start with a lowercase ASCII letter, found '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in service ID")
            }

            Self::InvalidSegmentEnd => {
                write!(f, "service ID segment must not end with '-'")
            }
        }
    }
}

impl Error for ServiceIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_rumahl_service_id() {
        let id = ServiceId::parse("rumahl.storage").unwrap();

        assert_eq!(id.as_str(), "rumahl.storage");
    }

    #[test]
    fn accepts_multi_segment_service_id() {
        assert!(
            ServiceId::parse("com.example.special-service").is_ok()
        );
    }

    #[test]
    fn rejects_single_segment_service_id() {
        assert_eq!(
            ServiceId::parse("storage").unwrap_err(),
            ServiceIdError::TooFewSegments
        );
    }

    #[test]
    fn rejects_uppercase_service_id() {
        assert_eq!(
            ServiceId::parse("rumahl.Storage").unwrap_err(),
            ServiceIdError::InvalidSegmentStart('S')
        );
    }
}