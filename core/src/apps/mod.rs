mod contribution;
mod installed;
mod manifest;
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
