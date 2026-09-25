use std::error::Error;
use std::fmt;

use rumahl_core::{AccountStateRepository, UserId};
use rumahl_ui_contracts::ShellUser;

use crate::{BoxedShellSourceError, ShellSnapshotSubject, ShellUserSource};

pub const DEFAULT_SHELL_LOCALE: &str = "en-US";

pub trait ShellLocalePreferenceSource {
    fn load_locale(&self, user_id: &UserId) -> Result<Option<String>, BoxedShellSourceError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultEnglishLocale;

impl ShellLocalePreferenceSource for DefaultEnglishLocale {
    fn load_locale(&self, _: &UserId) -> Result<Option<String>, BoxedShellSourceError> {
        Ok(None)
    }
}

pub struct AccountStateShellUserSource<R, L> {
    accounts: R,
    locales: L,
}

impl<R, L> AccountStateShellUserSource<R, L> {
    pub fn new(accounts: R, locales: L) -> Self {
        Self { accounts, locales }
    }
}

impl<R, L> ShellUserSource for AccountStateShellUserSource<R, L>
where
    R: AccountStateRepository,
    L: ShellLocalePreferenceSource,
{
    fn load_user(
        &self,
        subject: &ShellSnapshotSubject,
    ) -> Result<ShellUser, BoxedShellSourceError> {
        let state = self.accounts.load()?;
        let account = state
            .accounts()
            .get(subject.user_id())
            .filter(|account| account.can_authenticate())
            .ok_or(AccountProfileUnavailable)?;
        let locale = self
            .locales
            .load_locale(subject.user_id())?
            .unwrap_or_else(|| DEFAULT_SHELL_LOCALE.to_owned());
        Ok(ShellUser::new(account.display_name(), locale)?)
    }
}

#[derive(Debug, Clone, Copy)]
struct AccountProfileUnavailable;

impl fmt::Display for AccountProfileUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "account profile is unavailable")
    }
}

impl Error for AccountProfileUnavailable {}

#[cfg(test)]
mod tests {
    use std::io;

    use rumahl_core::{AccountState, AccountStateRepository, CorrelationId};

    use super::*;

    struct MemoryAccounts(AccountState);

    impl AccountStateRepository for MemoryAccounts {
        type Error = io::Error;

        fn load(&self) -> Result<AccountState, Self::Error> {
            Ok(self.0.clone())
        }

        fn store(&self, _: &AccountState) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct PreferredGerman;

    impl ShellLocalePreferenceSource for PreferredGerman {
        fn load_locale(&self, _: &UserId) -> Result<Option<String>, BoxedShellSourceError> {
            Ok(Some("de-DE".to_owned()))
        }
    }

    #[test]
    fn projects_real_account_name_with_english_default_or_german_preference() {
        let mut state = AccountState::new();
        let user_id = state.create_account("ada", "Ada Example").unwrap();
        let subject = ShellSnapshotSubject::new(user_id, CorrelationId::new());

        let english =
            AccountStateShellUserSource::new(MemoryAccounts(state.clone()), DefaultEnglishLocale)
                .load_user(&subject)
                .unwrap();
        let german = AccountStateShellUserSource::new(MemoryAccounts(state), PreferredGerman)
            .load_user(&subject)
            .unwrap();

        assert_eq!(english.display_name(), "Ada Example");
        assert_eq!(english.locale(), "en-US");
        assert_eq!(german.locale(), "de-DE");
    }

    #[test]
    fn never_projects_a_different_users_profile() {
        let mut state = AccountState::new();
        state.create_account("ada", "Ada Example").unwrap();
        let source = AccountStateShellUserSource::new(MemoryAccounts(state), DefaultEnglishLocale);

        let error = source
            .load_user(&ShellSnapshotSubject::new(
                UserId::new(),
                CorrelationId::new(),
            ))
            .unwrap_err();

        assert_eq!(error.to_string(), "account profile is unavailable");
    }
}
