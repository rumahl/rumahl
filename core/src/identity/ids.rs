use std::error::Error;
use std::fmt;

/// Stable identifier of an app inside the rumahl platform.
///
/// App IDs use a reverse-domain-like format, for example:
///
/// - `com.rumahl.notes`
/// - `org.jellyfin.server`
///
/// Creating an `AppId` is only possible through validation, so invalid IDs
/// cannot accidentally enter the domain model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl AppId {
    pub const MAX_LENGTH: usize = 255;
    pub const MAX_SEGMENT_LENGTH: usize = 63;
    pub const MIN_SEGMENTS: usize = 3;

    /// Parse and validate an app identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, AppIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(AppIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(AppIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < Self::MIN_SEGMENTS {
            return Err(AppIdError::TooFewSegments);
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

fn validate_segment(segment: &str) -> Result<(), AppIdError> {
    if segment.is_empty() {
        return Err(AppIdError::EmptySegment);
    }

    if segment.len() > AppId::MAX_SEGMENT_LENGTH {
        return Err(AppIdError::SegmentTooLong);
    }

    let mut characters = segment.chars();

    // Safe because the empty segment case was handled above.
    let first = characters
        .next()
        .expect("non-empty app ID segment must contain a first character");

    if !first.is_ascii_lowercase() {
        return Err(AppIdError::InvalidSegmentStart(first));
    }

    for character in characters {
        if !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && character != '-'
        {
            return Err(AppIdError::InvalidCharacter(character));
        }
    }

    if segment.ends_with('-') {
        return Err(AppIdError::InvalidSegmentEnd);
    }

    Ok(())
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for AppId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for AppId {
    type Error = AppIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for AppId {
    type Error = AppIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for AppIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "app ID must not be empty"),
            Self::TooLong => write!(
                f,
                "app ID must not exceed {} characters",
                AppId::MAX_LENGTH
            ),
            Self::TooFewSegments => write!(
                f,
                "app ID must contain at least {} segments",
                AppId::MIN_SEGMENTS
            ),
            Self::EmptySegment => write!(f, "app ID must not contain empty segments"),
            Self::SegmentTooLong => write!(
                f,
                "app ID segment must not exceed {} characters",
                AppId::MAX_SEGMENT_LENGTH
            ),
            Self::InvalidSegmentStart(character) => write!(
                f,
                "app ID segment must start with a lowercase ASCII letter, found '{character}'"
            ),
            Self::InvalidCharacter(character) => {
                write!(f, "invalid character '{character}' in app ID")
            }
            Self::InvalidSegmentEnd => write!(f, "app ID segment must not end with '-'"),
        }
    }
}

impl Error for AppIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_app_id() {
        let id = AppId::parse("com.rumahl.notes").unwrap();

        assert_eq!(id.as_str(), "com.rumahl.notes");
    }

    #[test]
    fn accepts_digits_after_first_character() {
        assert!(AppId::parse("com.rumahl.notes2").is_ok());
    }

    #[test]
    fn accepts_hyphens_inside_segment() {
        assert!(AppId::parse("com.rumahl.my-notes").is_ok());
    }

    #[test]
    fn rejects_empty_app_id() {
        assert_eq!(AppId::parse("").unwrap_err(), AppIdError::Empty);
    }

    #[test]
    fn rejects_too_few_segments() {
        assert_eq!(
            AppId::parse("rumahl.notes").unwrap_err(),
            AppIdError::TooFewSegments
        );
    }

    #[test]
    fn rejects_empty_segment() {
        assert_eq!(
            AppId::parse("com..notes").unwrap_err(),
            AppIdError::EmptySegment
        );
    }

    #[test]
    fn rejects_uppercase_segment_start() {
        assert_eq!(
            AppId::parse("com.rumahl.Notes").unwrap_err(),
            AppIdError::InvalidSegmentStart('N')
        );
    }

    #[test]
    fn rejects_uppercase_inside_segment() {
        assert_eq!(
            AppId::parse("com.rumahl.noTes").unwrap_err(),
            AppIdError::InvalidCharacter('T')
        );
    }

    #[test]
    fn rejects_invalid_character() {
        assert_eq!(
            AppId::parse("com.rumahl.notes_test").unwrap_err(),
            AppIdError::InvalidCharacter('_')
        );
    }

    #[test]
    fn rejects_segment_starting_with_digit() {
        assert_eq!(
            AppId::parse("com.rumahl.2notes").unwrap_err(),
            AppIdError::InvalidSegmentStart('2')
        );
    }

    #[test]
    fn rejects_segment_ending_with_hyphen() {
        assert_eq!(
            AppId::parse("com.rumahl.notes-").unwrap_err(),
            AppIdError::InvalidSegmentEnd
        );
    }

    #[test]
    fn supports_try_from_str() {
        let id = AppId::try_from("com.rumahl.notes").unwrap();

        assert_eq!(id.to_string(), "com.rumahl.notes");
    }
}
