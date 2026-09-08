use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl CapabilityId {
    pub fn parse(value: impl Into<String>) -> Result<Self, CapabilityIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(CapabilityIdError::Empty);
        }

        if value.len() > 255 {
            return Err(CapabilityIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < 3 {
            return Err(CapabilityIdError::TooFewSegments);
        }

        for segment in segments {
            if segment.is_empty() {
                return Err(CapabilityIdError::EmptySegment);
            }

            if segment.len() > 63 {
                return Err(CapabilityIdError::SegmentTooLong);
            }

            let mut chars = segment.chars();

            let first = chars
                .next()
                .expect("non-empty segment must have first character");

            if !first.is_ascii_lowercase() {
                return Err(
                    CapabilityIdError::InvalidSegmentStart(first)
                );
            }

            for character in chars {
                if !character.is_ascii_lowercase()
                    && !character.is_ascii_digit()
                    && character != '-'
                {
                    return Err(
                        CapabilityIdError::InvalidCharacter(character)
                    );
                }
            }

            if segment.ends_with('-') {
                return Err(
                    CapabilityIdError::InvalidSegmentEnd
                );
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CapabilityId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for CapabilityId {
    type Error = CapabilityIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for CapabilityId {
    type Error = CapabilityIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for CapabilityIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "capability id cannot be empty")
            }

            Self::TooLong => {
                write!(f, "capability id is too long")
            }

            Self::TooFewSegments => {
                write!(
                    f,
                    "capability id must contain at least three segments"
                )
            }

            Self::EmptySegment => {
                write!(
                    f,
                    "capability id cannot contain empty segments"
                )
            }

            Self::SegmentTooLong => {
                write!(
                    f,
                    "capability id segment is too long"
                )
            }

            Self::InvalidSegmentStart(character) => {
                write!(
                    f,
                    "capability id segment cannot start with '{character}'"
                )
            }

            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "capability id contains invalid character '{character}'"
                )
            }

            Self::InvalidSegmentEnd => {
                write!(
                    f,
                    "capability id segment cannot end with '-'"
                )
            }
        }
    }
}

impl Error for CapabilityIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_platform_capability() {
        let id =
            CapabilityId::parse("rumahl.search.query").unwrap();

        assert_eq!(
            id.as_str(),
            "rumahl.search.query"
        );
    }

    #[test]
    fn accepts_third_party_capability() {
        let id =
            CapabilityId::parse(
                "com.rumahl.notes.search"
            )
            .unwrap();

        assert_eq!(
            id.as_str(),
            "com.rumahl.notes.search"
        );
    }

    #[test]
    fn rejects_too_few_segments() {
        assert_eq!(
            CapabilityId::parse("rumahl.search").unwrap_err(),
            CapabilityIdError::TooFewSegments
        );
    }

    #[test]
    fn rejects_uppercase_character() {
        assert_eq!(
            CapabilityId::parse(
                "rumahl.search.Query"
            )
            .unwrap_err(),
            CapabilityIdError::InvalidSegmentStart('Q')
        );
    }

    #[test]
    fn rejects_trailing_hyphen() {
        assert_eq!(
            CapabilityId::parse(
                "rumahl.search.query-"
            )
            .unwrap_err(),
            CapabilityIdError::InvalidSegmentEnd
        );
    }
}