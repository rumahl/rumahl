use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppDatabaseId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppDatabaseIdError {
    Empty,
    TooLong,
    InvalidStart(char),
    InvalidCharacter(char),
    InvalidEnd,
}

impl AppDatabaseId {
    pub const MAX_LENGTH: usize = 63;

    pub fn parse(value: impl Into<String>) -> Result<Self, AppDatabaseIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(AppDatabaseIdError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(AppDatabaseIdError::TooLong);
        }

        let mut characters = value.chars();
        let first = characters
            .next()
            .expect("non-empty app database id must have a first character");

        if !first.is_ascii_lowercase() {
            return Err(AppDatabaseIdError::InvalidStart(first));
        }

        for character in characters {
            if !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-' {
                return Err(AppDatabaseIdError::InvalidCharacter(character));
            }
        }

        if value.ends_with('-') {
            return Err(AppDatabaseIdError::InvalidEnd);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AppDatabaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for AppDatabaseId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for AppDatabaseId {
    type Error = AppDatabaseIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for AppDatabaseId {
    type Error = AppDatabaseIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for AppDatabaseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "app database id must not be empty"),
            Self::TooLong => write!(
                f,
                "app database id must not exceed {} bytes",
                AppDatabaseId::MAX_LENGTH
            ),
            Self::InvalidStart(character) => write!(
                f,
                "app database id must start with a lowercase ASCII letter, found '{character}'"
            ),
            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "app database id contains invalid character '{character}'"
                )
            }
            Self::InvalidEnd => write!(f, "app database id must not end with '-'"),
        }
    }
}

impl Error for AppDatabaseIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_logical_database_ids() {
        assert_eq!(AppDatabaseId::parse("primary").unwrap().as_str(), "primary");
        assert!(AppDatabaseId::parse("search-index-2").is_ok());
    }

    #[test]
    fn rejects_noncanonical_and_path_like_ids() {
        assert_eq!(
            AppDatabaseId::parse("PostgreSQL").unwrap_err(),
            AppDatabaseIdError::InvalidStart('P')
        );
        assert_eq!(
            AppDatabaseId::parse("data/main").unwrap_err(),
            AppDatabaseIdError::InvalidCharacter('/')
        );
        assert_eq!(
            AppDatabaseId::parse("main.db").unwrap_err(),
            AppDatabaseIdError::InvalidCharacter('.')
        );
    }
}
