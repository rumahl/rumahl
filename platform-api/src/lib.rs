//! Transport-neutral request boundary for rumahl.
//!
//! HTTP, WebSocket, IPC, and future SDK adapters keep their credential and DTO
//! types outside this crate. After successful authentication this crate creates
//! the trusted `OperationContext` consumed by platform services.

mod authentication;
mod gateway;
mod local_session;
mod request;
mod shell_snapshot;

pub use authentication::{AuthenticatedPrincipal, RequestAuthenticator, RequestContextFactory};
pub use gateway::{PlatformRequestAuthenticationError, PlatformRequestGateway};
pub use local_session::{
    AuthenticationClock, LocalSessionAuthenticationError, LocalSessionAuthenticator,
    SessionCredentialResolver, SystemAuthenticationClock,
};
pub use request::PlatformRequest;
pub use shell_snapshot::{
    AuthenticatedShellSnapshotService, ShellSnapshotProvider, ShellSnapshotQuery,
    ShellSnapshotRequestError, ShellSnapshotSubject,
};
