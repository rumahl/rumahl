use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublisherId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublisherIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl PublisherId {
    pub const MAX_LENGTH: usize = 255;
    pub const MAX_SEGMENT_LENGTH: usize = 63;
    pub const MIN_SEGMENTS: usize = 2;

    pub fn parse(value: impl Into<String>) -> Result<Self, PublisherIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(PublisherIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(PublisherIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < Self::MIN_SEGMENTS {
            return Err(PublisherIdError::TooFewSegments);
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

fn validate_segment(segment: &str) -> Result<(), PublisherIdError> {
    if segment.is_empty() {
        return Err(PublisherIdError::EmptySegment);
    }

    if segment.len() > PublisherId::MAX_SEGMENT_LENGTH {
        return Err(PublisherIdError::SegmentTooLong);
    }

    let mut characters = segment.chars();

    let first = characters
        .next()
        .expect("non-empty publisher ID segment must contain a first character");

    if !first.is_ascii_lowercase() {
        return Err(PublisherIdError::InvalidSegmentStart(first));
    }

    for character in characters {
        if !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && character != '-'
        {
            return Err(PublisherIdError::InvalidCharacter(character));
        }
    }

    if segment.ends_with('-') {
        return Err(PublisherIdError::InvalidSegmentEnd);
    }

    Ok(())
}

impl fmt::Display for PublisherId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for PublisherId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for PublisherId {
    type Error = PublisherIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for PublisherId {
    type Error = PublisherIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for PublisherIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "publisher ID must not be empty"),
            Self::TooLong => write!(
                f,
                "publisher ID must not exceed {} characters",
                PublisherId::MAX_LENGTH
            ),
            Self::TooFewSegments => write!(
                f,
                "publisher ID must contain at least {} segments",
                PublisherId::MIN_SEGMENTS
            ),
            Self::EmptySegment => write!(f, "publisher ID must not contain empty segments"),
            Self::SegmentTooLong => write!(
                f,
                "publisher ID segment must not exceed {} characters",
                PublisherId::MAX_SEGMENT_LENGTH
            ),
            Self::InvalidSegmentStart(character) => write!(
                f,
                "publisher ID segment must start with a lowercase ASCII letter, found '{character}'"
            ),
            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in publisher ID")
            }
            Self::InvalidSegmentEnd => write!(f, "publisher ID segment must not end with '-'"),
        }
    }
}

impl Error for PublisherIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_publisher_id() {
        let id = PublisherId::parse("com.rumahl").unwrap();

        assert_eq!(id.as_str(), "com.rumahl");
    }

    #[test]
    fn accepts_another_valid_publisher_id() {
        assert!(PublisherId::parse("org.jellyfin").is_ok());
    }

    #[test]
    fn rejects_single_segment() {
        assert_eq!(
            PublisherId::parse("rumahl").unwrap_err(),
            PublisherIdError::TooFewSegments
        );
    }
}