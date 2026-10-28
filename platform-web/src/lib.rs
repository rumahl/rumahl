//! Local HTTP transport for the shell. TLS termination and socket ownership are
//! deployment concerns; this crate never binds a public TCP listener.

mod backend;
mod events;
mod oidc;
mod sensors;
mod server;
mod ssr;
mod streams;

pub use sensors::{
    DiskDevice, Sensors, Storage, read_disks, read_sensors, read_storage, run_disk_test,
};

mod recovery_settings;
pub use recovery_settings::RecoverySettings;

mod recovery_tools;
pub use recovery_tools::{read_boot, read_logs, read_network, read_system, run_network_test};

pub use backend::{
    PlatformShellBackend, ShellAuthenticationError, ShellBackend, ShellBackendError, ShellIdentity,
};
pub use events::{InMemoryShellEvents, ShellEventSource};
pub use oidc::OidcGateway;
pub use server::{
    GatewayConfig, GatewayError, GatewayState, WidgetFrameResolver, router, serve, serve_router,
};
pub use streams::{
    RegisteredStreamProvider, StreamAccess, StreamDescriptor, StreamEndpoint, StreamEndpointError,
    StreamProvider, StreamProviderError,
};

mod browser_auth;
mod offline;
pub use browser_auth::{BrowserSessions, LoginAccount, SESSION_SECONDS};

mod apps;
pub use apps::{
    AppAccess, AppAccessError, AppAsset, AppData, AppDataEntry, AppProvider, AppSettingInfo,
    AppSettingOptionInfo, AppSettingsInfo, BrowserDelivery, CapabilityOutcome, CapabilityResource,
    CapabilityResult, CatalogApp, ImportedApp, PackageUploadFile, RuntimeAction, RuntimeState,
    app_error,
};

mod app_channel;
pub use app_channel::{
    CHANNEL_PROTOCOL, ChannelAuthenticator, ChannelFrame, ChannelTokenStore, SocketRuntimeChannel,
    read_hello, run_connection, serve_channel_listener,
};

mod preferences;

mod host_files;
pub use host_files::{HostArea, HostEntry, HostError, HostFiles};

mod os_mode;

mod files;
mod workspace;
