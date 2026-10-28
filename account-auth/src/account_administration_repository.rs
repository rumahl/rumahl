use std::error::Error;

use rumahl_core::{LocalAccount, SessionId, UnixTimestamp};

use crate::PasswordCredentialRecord;

pub trait LocalAccountAdministrationRepository {
    type Error: Error + Send + Sync + 'static;

    fn provision_password_account(
        &self,
        account: &LocalAccount,
        credential: &PasswordCredentialRecord,
    ) -> Result<(), Self::Error>;

    fn replace_password_and_revoke_sessions(
        &self,
        credential: &PasswordCredentialRecord,
        session_ids: &[SessionId],
        revoked_at: UnixTimestamp,
    ) -> Result<(), Self::Error>;
}
