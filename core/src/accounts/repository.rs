use std::error::Error;

use super::AccountState;

pub trait AccountStateRepository {
    type Error: Error + Send + Sync + 'static;

    fn load(&self) -> Result<AccountState, Self::Error>;

    fn store(&self, state: &AccountState) -> Result<(), Self::Error>;
}
