use std::error::Error;
use std::fmt;

use crate::UserId;

use super::{AccountUsername, LocalAccount};

#[derive(Debug, Default, Clone)]
pub struct AccountRegistry {
    accounts: Vec<LocalAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountRegistryError {
    UserAlreadyRegistered,
    UsernameAlreadyRegistered,
}

impl AccountRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, account: LocalAccount) -> Result<(), AccountRegistryError> {
        if self.get(account.user_id()).is_some() {
            return Err(AccountRegistryError::UserAlreadyRegistered);
        }

        if self.get_by_username(account.username()).is_some() {
            return Err(AccountRegistryError::UsernameAlreadyRegistered);
        }

        self.accounts.push(account);

        Ok(())
    }

    pub fn get(&self, user_id: &UserId) -> Option<&LocalAccount> {
        self.accounts
            .iter()
            .find(|account| account.user_id() == user_id)
    }

    pub fn get_by_username(&self, username: &AccountUsername) -> Option<&LocalAccount> {
        self.accounts
            .iter()
            .find(|account| account.username() == username)
    }

    pub fn accounts(&self) -> &[LocalAccount] {
        &self.accounts
    }

    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    pub(crate) fn get_mut(&mut self, user_id: &UserId) -> Option<&mut LocalAccount> {
        self.accounts
            .iter_mut()
            .find(|account| account.user_id() == user_id)
    }
}

impl fmt::Display for AccountRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserAlreadyRegistered => write!(f, "user is already registered"),
            Self::UsernameAlreadyRegistered => write!(f, "account username is already registered"),
        }
    }
}

impl Error for AccountRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_normalized_duplicate_username() {
        let mut registry = AccountRegistry::new();

        registry
            .register(LocalAccount::create("kai", "Kai").unwrap())
            .unwrap();

        assert_eq!(
            registry
                .register(LocalAccount::create("KAI", "Another Kai").unwrap())
                .unwrap_err(),
            AccountRegistryError::UsernameAlreadyRegistered
        );
    }
}
