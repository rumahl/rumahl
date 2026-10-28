use std::error::Error;
use std::fmt;

use crate::UserId;

use super::{AccountUsername, AccountUsernameError};

const MAX_DISPLAY_NAME_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Active,
    Locked,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAccount {
    user_id: UserId,
    username: AccountUsername,
    display_name: String,
    status: AccountStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalAccountError {
    InvalidUsername(AccountUsernameError),
    EmptyDisplayName,
    DisplayNameTooLong,
    DisplayNameContainsControlCharacter,
}

impl LocalAccount {
    pub fn create(username: &str, display_name: &str) -> Result<Self, LocalAccountError> {
        Self::restore(
            UserId::new(),
            AccountUsername::parse(username).map_err(LocalAccountError::InvalidUsername)?,
            display_name,
            AccountStatus::Active,
        )
    }

    pub fn restore(
        user_id: UserId,
        username: AccountUsername,
        display_name: &str,
        status: AccountStatus,
    ) -> Result<Self, LocalAccountError> {
        let display_name = display_name.trim();

        if display_name.is_empty() {
            return Err(LocalAccountError::EmptyDisplayName);
        }

        if display_name.chars().count() > MAX_DISPLAY_NAME_LENGTH {
            return Err(LocalAccountError::DisplayNameTooLong);
        }

        if display_name.chars().any(char::is_control) {
            return Err(LocalAccountError::DisplayNameContainsControlCharacter);
        }

        Ok(Self {
            user_id,
            username,
            display_name: display_name.to_owned(),
            status,
        })
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn username(&self) -> &AccountUsername {
        &self.username
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn status(&self) -> AccountStatus {
        self.status
    }

    pub fn can_authenticate(&self) -> bool {
        self.status == AccountStatus::Active
    }

    pub(crate) fn set_status(&mut self, status: AccountStatus) {
        self.status = status;
    }
}

impl fmt::Display for LocalAccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUsername(error) => write!(f, "invalid account username: {error}"),
            Self::EmptyDisplayName => write!(f, "account display name must not be empty"),
            Self::DisplayNameTooLong => write!(
                f,
                "account display name must contain at most {MAX_DISPLAY_NAME_LENGTH} characters"
            ),
            Self::DisplayNameContainsControlCharacter => {
                write!(
                    f,
                    "account display name must not contain control characters"
                )
            }
        }
    }
}

impl Error for LocalAccountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUsername(error) => Some(error),
            Self::EmptyDisplayName
            | Self::DisplayNameTooLong
            | Self::DisplayNameContainsControlCharacter => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_active_account() {
        let account = LocalAccount::create("kai", "Kai Meier").unwrap();

        assert_eq!(account.username().as_str(), "kai");
        assert_eq!(account.display_name(), "Kai Meier");
        assert_eq!(account.status(), AccountStatus::Active);
        assert!(account.can_authenticate());
    }

    #[test]
    fn trims_display_name() {
        let account = LocalAccount::create("kai", "  Kai Meier  ").unwrap();

        assert_eq!(account.display_name(), "Kai Meier");
    }

    #[test]
    fn rejects_control_characters_in_display_name() {
        assert_eq!(
            LocalAccount::create("kai", "Kai\nMeier").unwrap_err(),
            LocalAccountError::DisplayNameContainsControlCharacter
        );
    }
}
