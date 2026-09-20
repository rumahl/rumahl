use std::error::Error;
use std::fmt;

use crate::{AppIdentity, InstallationId};

use super::{AppManifest, AppManifestValidationError, AppManifestValidator};

#[derive(Debug, Clone)]
pub struct InstalledApp {
    identity: AppIdentity,
    manifest: AppManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstalledAppError {
    InvalidManifest(AppManifestValidationError),
    AppIdMismatch,
    PublisherIdMismatch,
}

impl InstalledApp {
    pub fn create(
        manifest: AppManifest,
        validator: &AppManifestValidator,
    ) -> Result<Self, InstalledAppError> {
        Self::validate_manifest(&manifest, validator)?;

        let identity = AppIdentity::new(
            manifest.app_id().clone(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

        Ok(Self { identity, manifest })
    }

    pub(crate) fn restore(
        identity: AppIdentity,
        manifest: AppManifest,
        validator: &AppManifestValidator,
    ) -> Result<Self, InstalledAppError> {
        Self::validate_manifest(&manifest, validator)?;

        if identity.app_id() != manifest.app_id() {
            return Err(InstalledAppError::AppIdMismatch);
        }

        if identity.publisher_id() != manifest.publisher_id() {
            return Err(InstalledAppError::PublisherIdMismatch);
        }

        Ok(Self { identity, manifest })
    }

    pub fn identity(&self) -> &AppIdentity {
        &self.identity
    }

    pub fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    pub fn installation_id(&self) -> &InstallationId {
        self.identity.installation_id()
    }

    fn validate_manifest(
        manifest: &AppManifest,
        validator: &AppManifestValidator,
    ) -> Result<(), InstalledAppError> {
        validator
            .validate(manifest)
            .map_err(InstalledAppError::InvalidManifest)
    }
}

impl fmt::Display for InstalledAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(error) => {
                write!(
                    f,
                    "cannot create installed app from invalid manifest: {error}"
                )
            }

            Self::AppIdMismatch => {
                write!(f, "installed app identity does not match manifest app id")
            }

            Self::PublisherIdMismatch => {
                write!(
                    f,
                    "installed app identity does not match manifest publisher id"
                )
            }
        }
    }
}

impl Error for InstalledAppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidManifest(error) => Some(error),
            Self::AppIdMismatch | Self::PublisherIdMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, CapabilityId, ContributionId, PublisherId, SearchContributionDeclaration,
    };

    fn web_runtime() -> crate::RuntimeDescriptor {
        let mut runtime = crate::RuntimeDescriptor::web();

        runtime
            .add_entrypoint(crate::RuntimeEntrypoint::web_asset(
                crate::RuntimeEntrypointId::parse("main").unwrap(),
                crate::PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        runtime
    }

    fn manifest() -> AppManifest {
        AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            web_runtime(),
        )
        .unwrap()
    }

    #[test]
    fn creates_installed_app_from_valid_manifest() {
        let manifest = manifest();

        let validator = AppManifestValidator::new();

        let installed = InstalledApp::create(manifest, &validator).unwrap();

        assert_eq!(installed.identity().app_id().as_str(), "com.rumahl.notes");

        assert_eq!(installed.manifest().display_name(), "Notes");
    }

    #[test]
    fn identity_is_derived_from_manifest() {
        let manifest = manifest();

        let app_id = manifest.app_id().clone();

        let publisher_id = manifest.publisher_id().clone();

        let validator = AppManifestValidator::new();

        let installed = InstalledApp::create(manifest, &validator).unwrap();

        assert_eq!(installed.identity().app_id(), &app_id);

        assert_eq!(installed.identity().publisher_id(), &publisher_id);
    }

    #[test]
    fn separate_installations_receive_different_ids() {
        let validator = AppManifestValidator::new();

        let first = InstalledApp::create(manifest(), &validator).unwrap();

        let second = InstalledApp::create(manifest(), &validator).unwrap();

        assert_ne!(first.installation_id(), second.installation_id());
    }

    #[test]
    fn rejects_invalid_manifest() {
        let mut manifest = manifest();

        manifest
            .add_contribution(
                SearchContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.search").unwrap(),
                    CapabilityId::parse("rumahl.search.query").unwrap(),
                )
                .into(),
            )
            .unwrap();

        /*
         * rumahl.search.query was not added to
         * provided_capabilities.
         */

        let validator = AppManifestValidator::new();

        let result = InstalledApp::create(manifest, &validator);

        assert!(matches!(
            result,
            Err(InstalledAppError::InvalidManifest(
                AppManifestValidationError::SearchCapabilityNotProvided { .. }
            ))
        ));
    }

    #[test]
    fn restores_existing_installation_identity() {
        let manifest = manifest();

        let identity = AppIdentity::new(
            manifest.app_id().clone(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

        let installation_id = *identity.installation_id();

        let restored =
            InstalledApp::restore(identity, manifest, &AppManifestValidator::new()).unwrap();

        assert_eq!(restored.installation_id(), &installation_id);
    }

    #[test]
    fn rejects_restored_identity_with_different_app_id() {
        let manifest = manifest();

        let identity = AppIdentity::new(
            AppId::parse("com.rumahl.files").unwrap(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

        assert_eq!(
            InstalledApp::restore(identity, manifest, &AppManifestValidator::new()).unwrap_err(),
            InstalledAppError::AppIdMismatch
        );
    }

    #[test]
    fn rejects_restored_identity_with_different_publisher() {
        let manifest = manifest();

        let identity = AppIdentity::new(
            manifest.app_id().clone(),
            InstallationId::new(),
            PublisherId::parse("com.example").unwrap(),
        );

        assert_eq!(
            InstalledApp::restore(identity, manifest, &AppManifestValidator::new()).unwrap_err(),
            InstalledAppError::PublisherIdMismatch
        );
    }
}
