use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventName(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventNameError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl EventName {
    pub fn parse(value: impl Into<String>) -> Result<Self, EventNameError> {
        let value = value.into();

        if value.is_empty() {
            return Err(EventNameError::Empty);
        }

        if value.len() > 255 {
            return Err(EventNameError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < 3 {
            return Err(EventNameError::TooFewSegments);
        }

        for segment in segments {
            if segment.is_empty() {
                return Err(EventNameError::EmptySegment);
            }

            if segment.len() > 63 {
                return Err(EventNameError::SegmentTooLong);
            }

            let mut chars = segment.chars();

            let first = chars
                .next()
                .expect("non-empty segment must have first character");

            if !first.is_ascii_lowercase() {
                return Err(EventNameError::InvalidSegmentStart(first));
            }

            for character in chars {
                if !character.is_ascii_lowercase()
                    && !character.is_ascii_digit()
                    && character != '-'
                {
                    return Err(EventNameError::InvalidCharacter(character));
                }
            }

            if segment.ends_with('-') {
                return Err(EventNameError::InvalidSegmentEnd);
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EventName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for EventName {
    type Error = EventNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for EventName {
    type Error = EventNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for EventNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "event name cannot be empty")
            }

            Self::TooLong => {
                write!(f, "event name is too long")
            }

            Self::TooFewSegments => {
                write!(f, "event name must contain at least three segments")
            }

            Self::EmptySegment => {
                write!(f, "event name cannot contain empty segments")
            }

            Self::SegmentTooLong => {
                write!(f, "event name segment is too long")
            }

            Self::InvalidSegmentStart(character) => {
                write!(f, "event name segment cannot start with '{character}'")
            }

            Self::InvalidCharacter(character) => {
                write!(f, "event name contains invalid character '{character}'")
            }

            Self::InvalidSegmentEnd => {
                write!(f, "event name segment cannot end with '-'")
            }
        }
    }
}

impl Error for EventNameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_event_name() {
        let name = EventName::parse("rumahl.files.changed").unwrap();

        assert_eq!(name.as_str(), "rumahl.files.changed");
    }

    #[test]
    fn rejects_event_name_with_too_few_segments() {
        assert_eq!(
            EventName::parse("files.changed").unwrap_err(),
            EventNameError::TooFewSegments
        );
    }

    #[test]
    fn rejects_uppercase_event_name() {
        assert_eq!(
            EventName::parse("rumahl.Files.changed").unwrap_err(),
            EventNameError::InvalidSegmentStart('F')
        );
    }

    #[test]
    fn rejects_invalid_event_character() {
        assert_eq!(
            EventName::parse("rumahl.files.file_changed").unwrap_err(),
            EventNameError::InvalidCharacter('_')
        );
    }
}
