mod contribution;
mod installed;
mod installed_registry;
mod lifecycle;
mod manifest;
mod oidc;
mod registrar;
mod registration;
mod stream_presentation;
mod validator;
mod version;

pub use contribution::{
    CommandContributionDeclaration, CommandContributionDeclarationError, ContributionDeclaration,
    SearchContributionDeclaration,
};

pub use manifest::{AppManifest, AppManifestError};

pub use stream_presentation::{
    PreferredStreamSize, STREAMING_ENGINE_CAPABILITY, StreamPresentation, StreamPresentationError,
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
