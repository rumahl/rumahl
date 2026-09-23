//! SQLite implementation of the `rumahl-core` platform snapshot repository.
//!
//! The adapter owns persistence encoding and database behavior. Domain types
//! remain in `rumahl-core` and every loaded value is reconstructed through
//! the public core constructors before platform recovery can commit it.

mod account_repository;
mod app_database_provider;
mod app_operation_repository;
mod identity_schema;
mod local_account_administration_repository;
mod oidc_access_token_store;
mod oidc_authorization_store;
mod oidc_client_repository;
mod oidc_refresh_token_store;
mod oidc_schema;
mod password_credential_repository;
mod repository;
mod secret_store;
mod session_credential_repository;
mod wire;

pub use account_repository::{SqliteAccountStateRepository, SqliteAccountStateRepositoryError};
pub use app_database_provider::{SqliteAppDatabaseProvider, SqliteAppDatabaseProviderError};
pub use app_operation_repository::{
    SqliteAppOperationRepository, SqliteAppOperationRepositoryError,
};
pub use local_account_administration_repository::{
    SqliteLocalAccountAdministrationRepository, SqliteLocalAccountAdministrationRepositoryError,
};
pub use oidc_access_token_store::{SqliteOidcAccessTokenStore, SqliteOidcAccessTokenStoreError};
pub use oidc_authorization_store::{
    SqliteOidcAuthorizationStore, SqliteOidcAuthorizationStoreError,
};
pub use oidc_client_repository::{SqliteOidcClientRepository, SqliteOidcClientRepositoryError};
pub use oidc_refresh_token_store::{SqliteOidcRefreshTokenStore, SqliteOidcRefreshTokenStoreError};
pub use password_credential_repository::{
    SqlitePasswordCredentialRepository, SqlitePasswordCredentialRepositoryError,
};
pub use repository::{SqliteSnapshotRepository, SqliteSnapshotRepositoryError};
pub use secret_store::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyIdError,
    SecretEncryptionKeyProvider, SqliteSecretStore, SqliteSecretStoreError,
};
pub use session_credential_repository::{
    SqliteSessionCredentialRepository, SqliteSessionCredentialRepositoryError,
};
