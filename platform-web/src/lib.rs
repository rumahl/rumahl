//! Local HTTP transport for the shell. TLS termination and socket ownership are
//! deployment concerns; this crate never binds a public TCP listener.

mod backend;
mod events;
mod oidc;
mod server;
mod ssr;
mod streams;

pub use backend::{
    PlatformShellBackend, ShellAuthenticationError, ShellBackend, ShellBackendError, ShellIdentity,
};
pub use events::{InMemoryShellEvents, ShellEventSource};
pub use oidc::OidcGateway;
pub use server::{GatewayConfig, GatewayError, GatewayState, WidgetFrameResolver, router, serve};
pub use streams::{
    RegisteredStreamProvider, StreamAccess, StreamDescriptor, StreamEndpoint, StreamEndpointError,
    StreamProvider, StreamProviderError,
};
