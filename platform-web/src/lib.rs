//! Local HTTP transport for the shell. TLS termination and socket ownership are
//! deployment concerns; this crate never binds a public TCP listener.

mod backend;
mod events;
mod server;
mod ssr;

pub use backend::{
    PlatformShellBackend, ShellAuthenticationError, ShellBackend, ShellBackendError, ShellIdentity,
};
pub use events::{InMemoryShellEvents, ShellEventSource};
pub use server::{GatewayConfig, GatewayError, GatewayState, WidgetFrameResolver, router, serve};
