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
}

impl InstalledApp {
    pub fn install(
        manifest: AppManifest,
        validator: &AppManifestValidator,
    ) -> Result<Self, InstalledAppError> {
        validator
            .validate(&manifest)
            .map_err(InstalledAppError::InvalidManifest)?;

        let identity = AppIdentity::new(
            manifest.app_id().clone(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

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
}

impl fmt::Display for InstalledAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(error) => {
                write!(f, "cannot install invalid app manifest: {error}")
            }
        }
    }
}

impl Error for InstalledAppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidManifest(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, CapabilityId, ContributionId, PublisherId, SearchContributionDeclaration,
    };

    fn manifest() -> AppManifest {
        AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap()
    }

    #[test]
    fn installs_valid_manifest() {
        let manifest = manifest();

        let validator = AppManifestValidator::new();

        let installed = InstalledApp::install(manifest, &validator).unwrap();

        assert_eq!(installed.identity().app_id().as_str(), "com.rumahl.notes");

        assert_eq!(installed.manifest().display_name(), "Notes");
    }

    #[test]
    fn identity_is_derived_from_manifest() {
        let manifest = manifest();

        let app_id = manifest.app_id().clone();

        let publisher_id = manifest.publisher_id().clone();

        let validator = AppManifestValidator::new();

        let installed = InstalledApp::install(manifest, &validator).unwrap();

        assert_eq!(installed.identity().app_id(), &app_id);

        assert_eq!(installed.identity().publisher_id(), &publisher_id);
    }

    #[test]
    fn separate_installations_receive_different_ids() {
        let validator = AppManifestValidator::new();

        let first = InstalledApp::install(manifest(), &validator).unwrap();

        let second = InstalledApp::install(manifest(), &validator).unwrap();

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

        let result = InstalledApp::install(manifest, &validator);

        assert!(matches!(
            result,
            Err(InstalledAppError::InvalidManifest(
                AppManifestValidationError::SearchCapabilityNotProvided { .. }
            ))
        ));
    }
}
