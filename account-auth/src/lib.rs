//! Local rumahl OS authentication primitives.
//!
//! This crate owns opaque session credentials. Login methods such as passwords
//! and passkeys remain separate adapters which establish an `AccountSession`.
//! A session token only proves possession of a transport credential; the
//! platform API still revalidates the current account and session state for
//! every request.

mod repository;
mod resolver;
mod token;

pub use repository::{SessionCredentialRecord, SessionCredentialRepository};
pub use resolver::{
    SessionCredentialIssuanceError, SessionCredentialIssuer, StoredSessionCredentialResolver,
    StoredSessionCredentialResolverError,
};
pub use token::{
    SessionToken, SessionTokenDigest, SessionTokenGenerationError, SessionTokenParseError,
};
