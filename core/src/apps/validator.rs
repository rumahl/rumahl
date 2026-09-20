use std::error::Error;
use std::fmt;

use crate::{CapabilityId, ContributionId, RuntimeEntrypointKind, RuntimeKind};

use super::{AppManifest, ContributionDeclaration};

#[derive(Debug, Default, Clone, Copy)]
pub struct AppManifestValidator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManifestValidationError {
    MissingRuntimeEntrypoint {
        runtime: RuntimeKind,
        required: RuntimeEntrypointKind,
    },
    NativeRuntimeUnsupported,
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
        self.validate_runtime(manifest)?;

        for contribution in manifest.contributions() {
            self.validate_contribution(manifest, contribution)?;
        }

        Ok(())
    }

    fn validate_runtime(&self, manifest: &AppManifest) -> Result<(), AppManifestValidationError> {
        let runtime = manifest.runtime();

        let required = match runtime.kind() {
            RuntimeKind::Web => RuntimeEntrypointKind::WebAsset,
            RuntimeKind::Container => RuntimeEntrypointKind::ContainerArtifact,
            RuntimeKind::Native => {
                return Err(AppManifestValidationError::NativeRuntimeUnsupported);
            }
        };

        if runtime
            .entrypoints()
            .iter()
            .any(|entrypoint| entrypoint.kind() == required)
        {
            return Ok(());
        }

        Err(AppManifestValidationError::MissingRuntimeEntrypoint {
            runtime: runtime.kind(),
            required,
        })
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
            Self::MissingRuntimeEntrypoint { runtime, required } => {
                write!(
                    f,
                    "runtime '{runtime:?}' requires an entrypoint of kind '{required:?}'"
                )
            }

            Self::NativeRuntimeUnsupported => {
                write!(
                    f,
                    "native runtime installation requires a trust policy that is not available"
                )
            }

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
        ContributionId, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEndpointId,
        RuntimeEntrypoint, RuntimeEntrypointId, SearchContributionDeclaration,
    };

    fn manifest() -> AppManifest {
        let mut runtime = RuntimeDescriptor::web();

        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
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
    fn rejects_web_runtime_without_asset_entrypoint() {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            RuntimeDescriptor::web(),
        )
        .unwrap();

        assert_eq!(
            AppManifestValidator::new().validate(&manifest).unwrap_err(),
            AppManifestValidationError::MissingRuntimeEntrypoint {
                runtime: RuntimeKind::Web,
                required: RuntimeEntrypointKind::WebAsset,
            }
        );
    }

    #[test]
    fn rejects_container_runtime_without_artifact_entrypoint() {
        let mut runtime = RuntimeDescriptor::container();

        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();

        let manifest = AppManifest::new(
            AppId::parse("org.jellyfin.server").unwrap(),
            PublisherId::parse("org.jellyfin").unwrap(),
            AppVersion::new(1, 0, 0),
            "Jellyfin",
            runtime,
        )
        .unwrap();

        assert_eq!(
            AppManifestValidator::new().validate(&manifest).unwrap_err(),
            AppManifestValidationError::MissingRuntimeEntrypoint {
                runtime: RuntimeKind::Container,
                required: RuntimeEntrypointKind::ContainerArtifact,
            }
        );
    }

    #[test]
    fn rejects_native_runtime_until_trust_policy_exists() {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.native-helper").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Native Helper",
            RuntimeDescriptor::native(),
        )
        .unwrap();

        assert_eq!(
            AppManifestValidator::new().validate(&manifest).unwrap_err(),
            AppManifestValidationError::NativeRuntimeUnsupported
        );
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
