use std::error::Error;
use std::fmt;

use crate::{CapabilityId, ContributionId};

use super::{AppManifest, ContributionDeclaration};

#[derive(Debug, Default, Clone, Copy)]
pub struct AppManifestValidator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManifestValidationError {
    SearchCapabilityNotProvided {
        contribution: ContributionId,
        capability: CapabilityId,
    },
}

impl AppManifestValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, manifest: &AppManifest) -> Result<(), AppManifestValidationError> {
        for contribution in manifest.contributions() {
            self.validate_contribution(manifest, contribution)?;
        }

        Ok(())
    }

    fn validate_contribution(
        &self,
        manifest: &AppManifest,
        contribution: &ContributionDeclaration,
    ) -> Result<(), AppManifestValidationError> {
        match contribution {
            ContributionDeclaration::Command(_) => Ok(()),

            ContributionDeclaration::Search(search) => {
                if manifest
                    .provided_capabilities()
                    .iter()
                    .any(|capability| capability == search.capability())
                {
                    return Ok(());
                }

                Err(AppManifestValidationError::SearchCapabilityNotProvided {
                    contribution: search.id().clone(),
                    capability: search.capability().clone(),
                })
            }
        }
    }
}

impl fmt::Display for AppManifestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SearchCapabilityNotProvided {
                contribution,
                capability,
            } => {
                write!(
                    f,
                    "search contribution '{contribution}' references capability '{capability}' that the app does not provide"
                )
            }
        }
    }
}

impl Error for AppManifestValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, CapabilityId, CommandAction, CommandContributionDeclaration,
        ContributionId, PublisherId, SearchContributionDeclaration,
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
    fn accepts_manifest_without_contributions() {
        let manifest = manifest();

        let validator = AppManifestValidator::new();

        assert!(validator.validate(&manifest).is_ok());
    }

    #[test]
    fn accepts_search_contribution_with_provided_capability() {
        let mut manifest = manifest();

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        manifest
            .add_provided_capability(capability.clone())
            .unwrap();

        manifest
            .add_contribution(
                SearchContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.search").unwrap(),
                    capability,
                )
                .into(),
            )
            .unwrap();

        let validator = AppManifestValidator::new();

        assert!(validator.validate(&manifest).is_ok());
    }

    #[test]
    fn rejects_search_contribution_without_provided_capability() {
        let mut manifest = manifest();

        let contribution_id = ContributionId::parse("com.rumahl.notes.search").unwrap();

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        manifest
            .add_contribution(
                SearchContributionDeclaration::new(contribution_id.clone(), capability.clone())
                    .into(),
            )
            .unwrap();

        let validator = AppManifestValidator::new();

        assert_eq!(
            validator.validate(&manifest).unwrap_err(),
            AppManifestValidationError::SearchCapabilityNotProvided {
                contribution: contribution_id,
                capability,
            }
        );
    }

    #[test]
    fn command_may_invoke_capability_not_provided_by_app() {
        let mut manifest = manifest();

        manifest
            .add_contribution(
                CommandContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.open-file").unwrap(),
                    "Open file",
                    CommandAction::invoke_capability(
                        CapabilityId::parse("rumahl.files.open").unwrap(),
                    ),
                )
                .unwrap()
                .into(),
            )
            .unwrap();

        let validator = AppManifestValidator::new();

        assert!(validator.validate(&manifest).is_ok());
    }
}
