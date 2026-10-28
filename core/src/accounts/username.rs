use std::error::Error;
use std::fmt;

const MAX_USERNAME_LENGTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountUsername(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountUsernameError {
    Empty,
    TooLong,
    InvalidStart,
    InvalidEnd,
    InvalidCharacter(char),
}

impl AccountUsername {
    pub fn parse(value: &str) -> Result<Self, AccountUsernameError> {
        let normalized = value.trim().to_ascii_lowercase();

        if normalized.is_empty() {
            return Err(AccountUsernameError::Empty);
        }

        if normalized.len() > MAX_USERNAME_LENGTH {
            return Err(AccountUsernameError::TooLong);
        }

        let mut characters = normalized.chars();
        let first = characters.next().expect("non-empty username");

        if !first.is_ascii_alphanumeric() {
            return Err(AccountUsernameError::InvalidStart);
        }

        let last = normalized.chars().next_back().expect("non-empty username");

        if !last.is_ascii_alphanumeric() {
            return Err(AccountUsernameError::InvalidEnd);
        }

        if let Some(character) = normalized.chars().find(|character| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '_' | '-')
        }) {
            return Err(AccountUsernameError::InvalidCharacter(character));
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountUsername {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for AccountUsernameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "account username must not be empty"),
            Self::TooLong => write!(
                f,
                "account username must contain at most {MAX_USERNAME_LENGTH} ASCII characters"
            ),
            Self::InvalidStart => write!(f, "account username must start with a letter or number"),
            Self::InvalidEnd => write!(f, "account username must end with a letter or number"),
            Self::InvalidCharacter(character) => write!(
                f,
                "account username contains unsupported character '{character}'"
            ),
        }
    }
}

impl Error for AccountUsernameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_normalizes_username() {
        let username = AccountUsername::parse("  Kai.Meier  ").unwrap();

        assert_eq!(username.as_str(), "kai.meier");
    }

    #[test]
    fn accepts_supported_separators() {
        assert!(AccountUsername::parse("kai_meier-2").is_ok());
    }

    #[test]
    fn rejects_separator_at_boundaries() {
        assert_eq!(
            AccountUsername::parse(".kai").unwrap_err(),
            AccountUsernameError::InvalidStart
        );
        assert_eq!(
            AccountUsername::parse("kai-").unwrap_err(),
            AccountUsernameError::InvalidEnd
        );
    }

    #[test]
    fn rejects_non_ascii_username() {
        assert_eq!(
            AccountUsername::parse("käi").unwrap_err(),
            AccountUsernameError::InvalidCharacter('ä')
        );
    }
}
