mod contribution;
mod installed;
mod installed_registry;
mod lifecycle;
mod manifest;
mod registrar;
mod registration;
mod validator;
mod version;

pub use contribution::{
    CommandContributionDeclaration, CommandContributionDeclarationError, ContributionDeclaration,
    SearchContributionDeclaration,
};

pub use manifest::{AppManifest, AppManifestError};

pub use version::{AppVersion, AppVersionError};

pub use validator::{AppManifestValidationError, AppManifestValidator};

pub use installed::{InstalledApp, InstalledAppError};

pub use registration::{PlatformRegistration, PlatformRegistrationError};

pub use registrar::{PlatformDeregistrationReport, PlatformRegistrar, PlatformRegistrarError};

pub use installed_registry::{InstalledAppRegistry, InstalledAppRegistryError};

pub use lifecycle::{AppLifecycle, AppLifecycleError, AppUninstallResult};
