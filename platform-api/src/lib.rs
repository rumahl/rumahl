//! Transport-neutral request boundary for rumahl.
//!
//! HTTP, WebSocket, IPC, and future SDK adapters keep their credential and DTO
//! types outside this crate. After successful authentication this crate creates
//! the trusted `OperationContext` consumed by platform services.

mod authentication;
mod gateway;
mod request;

pub use authentication::{AuthenticatedPrincipal, RequestAuthenticator, RequestContextFactory};
pub use gateway::{PlatformRequestAuthenticationError, PlatformRequestGateway};
pub use request::PlatformRequest;
