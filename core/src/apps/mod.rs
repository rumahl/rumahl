mod connector;
mod contribution;
mod installed;
mod installed_registry;
mod lifecycle;
mod manifest;
mod oidc;
mod registrar;
mod registration;
mod settings;
mod stream_presentation;
mod surface;
mod validator;
mod version;

pub use contribution::{
    CommandContributionDeclaration, CommandContributionDeclarationError, ContributionDeclaration,
    SearchContributionDeclaration,
};

pub use manifest::{AppManifest, AppManifestError, RuntimeLifecycle};

/// Permission that lets an installed app ask the OS to make an HTTP(S)
/// request to an origin the user explicitly approved.
pub const APP_NETWORK_PERMISSION: &str = "com.rumahl.os.network";

pub use connector::{ConnectorDeclaration, ConnectorTarget, ConnectorTargetError};

pub use settings::{
    AppSettingDeclaration, AppSettingError, AppSettingKey, AppSettingKind, AppSettingOption,
    MAX_APP_SETTINGS,
};

pub use stream_presentation::{
    PreferredStreamSize, STREAMING_ENGINE_CAPABILITY, StreamPresentation, StreamPresentationError,
};

pub use surface::{
    SurfaceDeclaration, SurfaceIcon, SurfaceKind, SurfacePlacement, SurfaceSize, SurfaceSlot,
    SurfaceTone,
};

pub use oidc::{
    OidcCallbackPath, OidcCallbackPathError, OidcClientDeclaration, OidcClientDeclarationError,
    OidcClientType, OidcScope,
};

pub use version::{AppVersion, AppVersionError};

pub use validator::{AppManifestValidationError, AppManifestValidator};

pub use installed::{InstalledApp, InstalledAppError};

pub use registration::{PlatformRegistration, PlatformRegistrationError};

pub use registrar::{PlatformDeregistrationReport, PlatformRegistrar, PlatformRegistrarError};

pub use installed_registry::{InstalledAppRegistry, InstalledAppRegistryError};

pub use lifecycle::{AppLifecycle, AppLifecycleError, AppUninstallResult};
