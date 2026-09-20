//! Local rumahl OS authentication primitives.
//!
//! This crate owns opaque session credentials. Login methods such as passwords
//! and passkeys remain separate adapters which establish an `AccountSession`.
//! A session token only proves possession of a transport credential; the
//! platform API still revalidates the current account and session state for
//! every request.

mod password;
mod password_authentication;
mod password_repository;
mod repository;
mod resolver;
mod token;

pub use password::{
    Argon2idPasswordEngine, Password, PasswordBlocklist, PasswordEngineError, PasswordHashRecord,
    PasswordHashRecordError, PasswordPolicy, PasswordPolicyError,
};
pub use password_authentication::{
    PasswordAuthenticationError, PasswordAuthenticationService,
    PasswordAuthenticationServiceInitializationError, PasswordEnrollmentError,
    VerifiedLocalAccount,
};
pub use password_repository::{
    PasswordAttempt, PasswordAttemptPolicy, PasswordAttemptPolicyError, PasswordCredentialRecord,
    PasswordCredentialRepository,
};
pub use repository::{SessionCredentialRecord, SessionCredentialRepository};
pub use resolver::{
    SessionCredentialIssuanceError, SessionCredentialIssuer, StoredSessionCredentialResolver,
    StoredSessionCredentialResolverError,
};
pub use token::{
    SessionToken, SessionTokenDigest, SessionTokenGenerationError, SessionTokenParseError,
};
