//! SQLite implementation of the `rumahl-core` platform snapshot repository.
//!
//! The adapter owns persistence encoding and database behavior. Domain types
//! remain in `rumahl-core` and every loaded value is reconstructed through
//! the public core constructors before platform recovery can commit it.

mod account_repository;
mod repository;
mod session_credential_repository;
mod wire;

pub use account_repository::{SqliteAccountStateRepository, SqliteAccountStateRepositoryError};
pub use repository::{SqliteSnapshotRepository, SqliteSnapshotRepositoryError};
pub use session_credential_repository::{
    SqliteSessionCredentialRepository, SqliteSessionCredentialRepositoryError,
};
