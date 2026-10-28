//! Transport-neutral request boundary for rumahl.
//!
//! HTTP, WebSocket, IPC, and future SDK adapters keep their credential and DTO
//! types outside this crate. After successful authentication this crate creates
//! the trusted `OperationContext` consumed by platform services.

mod account_shell_user;
mod authentication;
mod gateway;
mod local_session;
mod request;
mod shell_composer;
mod shell_snapshot;

pub use account_shell_user::{
    AccountStateShellUserSource, DefaultEnglishLocale, ShellLocalePreferenceSource,
};
pub use authentication::{AuthenticatedPrincipal, RequestAuthenticator, RequestContextFactory};
pub use gateway::{PlatformRequestAuthenticationError, PlatformRequestGateway};
pub use local_session::{
    AuthenticationClock, LocalSessionAuthenticationError, LocalSessionAuthenticator,
    SessionCredentialResolver, SystemAuthenticationClock,
};
pub use request::PlatformRequest;
pub use shell_composer::{
    BoxedShellSourceError, ShellContributionSource, ShellRevisionSource, ShellSnapshotComposer,
    ShellSnapshotCompositionError, ShellSnapshotSourceKind, ShellStatusSource, ShellThemeSource,
    ShellUserSource,
};
pub use shell_snapshot::{
    AuthenticatedShellSnapshotService, ShellSnapshotProvider, ShellSnapshotQuery,
    ShellSnapshotRequestError, ShellSnapshotSubject,
};
