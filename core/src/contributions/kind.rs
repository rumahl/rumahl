use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContributionKind(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionKindError {
    Empty,
    TooLong,
    InvalidStart(char),
    InvalidCharacter(char),
    InvalidEnd,
}

impl ContributionKind {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContributionKindError> {
        let value = value.into();

        if value.is_empty() {
            return Err(ContributionKindError::Empty);
        }

        if value.len() > 64 {
            return Err(ContributionKindError::TooLong);
        }

        let mut chars = value.chars();

        let first = chars
            .next()
            .expect("non-empty contribution kind must have first character");

        if !first.is_ascii_lowercase() {
            return Err(ContributionKindError::InvalidStart(first));
        }

        for character in chars {
            if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
                return Err(ContributionKindError::InvalidCharacter(character));
            }
        }

        if value.ends_with('-') {
            return Err(ContributionKindError::InvalidEnd);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContributionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContributionKind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ContributionKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "contribution kind cannot be empty")
            }

            Self::TooLong => {
                write!(f, "contribution kind is too long")
            }

            Self::InvalidStart(character) => {
                write!(f, "contribution kind cannot start with '{character}'")
            }

            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "contribution kind contains invalid character '{character}'"
                )
            }

            Self::InvalidEnd => {
                write!(f, "contribution kind cannot end with '-'")
            }
        }
    }
}

impl Error for ContributionKindError {}
