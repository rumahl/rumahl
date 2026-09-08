use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContributionId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionIdError {
    Empty,
    TooLong,
    TooFewSegments,
    EmptySegment,
    SegmentTooLong,
    InvalidSegmentStart(char),
    InvalidCharacter(char),
    InvalidSegmentEnd,
}

impl ContributionId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContributionIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ContributionIdError::Empty);
        }

        if value.len() > 255 {
            return Err(ContributionIdError::TooLong);
        }

        let segments: Vec<&str> = value.split('.').collect();

        if segments.len() < 3 {
            return Err(ContributionIdError::TooFewSegments);
        }

        for segment in segments {
            if segment.is_empty() {
                return Err(ContributionIdError::EmptySegment);
            }

            if segment.len() > 63 {
                return Err(ContributionIdError::SegmentTooLong);
            }

            let mut chars = segment.chars();

            let first = chars
                .next()
                .expect("non-empty segment must have first character");

            if !first.is_ascii_lowercase() {
                return Err(ContributionIdError::InvalidSegmentStart(first));
            }

            for character in chars {
                if !character.is_ascii_lowercase()
                    && !character.is_ascii_digit()
                    && character != '-'
                {
                    return Err(ContributionIdError::InvalidCharacter(character));
                }
            }

            if segment.ends_with('-') {
                return Err(ContributionIdError::InvalidSegmentEnd);
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContributionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContributionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for ContributionId {
    type Error = ContributionIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ContributionId {
    type Error = ContributionIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for ContributionIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "contribution id cannot be empty")
            }

            Self::TooLong => {
                write!(f, "contribution id is too long")
            }

            Self::TooFewSegments => {
                write!(f, "contribution id must contain at least three segments")
            }

            Self::EmptySegment => {
                write!(f, "contribution id cannot contain empty segments")
            }

            Self::SegmentTooLong => {
                write!(f, "contribution id segment is too long")
            }

            Self::InvalidSegmentStart(character) => {
                write!(f, "contribution id segment cannot start with '{character}'")
            }

            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "contribution id contains invalid character '{character}'"
                )
            }

            Self::InvalidSegmentEnd => {
                write!(f, "contribution id segment cannot end with '-'")
            }
        }
    }
}

impl Error for ContributionIdError {}
