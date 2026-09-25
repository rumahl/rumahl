use std::error::Error;
use std::fmt;

use crate::{
    CapabilityId, ContributionId, OidcClientType, RuntimeEntrypointId, RuntimeEntrypointKind,
    RuntimeKind,
};

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
    StreamRequiresContainerRuntime(RuntimeKind),
    StreamEntrypointMissing(RuntimeEntrypointId),
    StreamEntrypointIncompatible(RuntimeEntrypointId),
    SearchCapabilityNotProvided {
        contribution: ContributionId,
        capability: CapabilityId,
    },
    OidcClientTypeIncompatible {
        runtime: RuntimeKind,
        client_type: OidcClientType,
    },
    OidcCallbackEntrypointMissing(RuntimeEntrypointId),
    OidcCallbackEntrypointIncompatible {
        runtime: RuntimeKind,
        entrypoint: RuntimeEntrypointId,
        kind: RuntimeEntrypointKind,
    },
}

impl AppManifestValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, manifest: &AppManifest) -> Result<(), AppManifestValidationError> {
        self.validate_runtime(manifest)?;
        self.validate_stream_presentation(manifest)?;
        self.validate_oidc(manifest)?;

        for contribution in manifest.contributions() {
            self.validate_contribution(manifest, contribution)?;
        }

        Ok(())
    }

    fn validate_stream_presentation(
        &self,
        manifest: &AppManifest,
    ) -> Result<(), AppManifestValidationError> {
        let Some(stream) = manifest.stream_presentation() else {
            return Ok(());
        };
        let runtime = manifest.runtime();
        if runtime.kind() != RuntimeKind::Container {
            return Err(AppManifestValidationError::StreamRequiresContainerRuntime(
                runtime.kind(),
            ));
        }
        let entrypoint = runtime.entrypoint(stream.app_entrypoint()).ok_or_else(|| {
            AppManifestValidationError::StreamEntrypointMissing(stream.app_entrypoint().clone())
        })?;
        if entrypoint.kind() != RuntimeEntrypointKind::ContainerArtifact {
            return Err(AppManifestValidationError::StreamEntrypointIncompatible(
                entrypoint.id().clone(),
            ));
        }
        Ok(())
    }

    fn validate_oidc(&self, manifest: &AppManifest) -> Result<(), AppManifestValidationError> {
        let Some(declaration) = manifest.oidc_client() else {
            return Ok(());
        };
        let runtime = manifest.runtime();
        let expected_client_type = match runtime.kind() {
            RuntimeKind::Web => OidcClientType::Public,
            RuntimeKind::Container => OidcClientType::Confidential,
            RuntimeKind::Native => {
                return Err(AppManifestValidationError::NativeRuntimeUnsupported);
            }
        };

        if declaration.client_type() != expected_client_type {
            return Err(AppManifestValidationError::OidcClientTypeIncompatible {
                runtime: runtime.kind(),
                client_type: declaration.client_type(),
            });
        }

        let entrypoint = runtime
            .entrypoint(declaration.callback_entrypoint())
            .ok_or_else(|| {
                AppManifestValidationError::OidcCallbackEntrypointMissing(
                    declaration.callback_entrypoint().clone(),
                )
            })?;
        let expected_kind = match runtime.kind() {
            RuntimeKind::Web => RuntimeEntrypointKind::WebAsset,
            RuntimeKind::Container => RuntimeEntrypointKind::Endpoint,
            RuntimeKind::Native => {
                return Err(AppManifestValidationError::NativeRuntimeUnsupported);
            }
        };

        if entrypoint.kind() != expected_kind {
            return Err(
                AppManifestValidationError::OidcCallbackEntrypointIncompatible {
                    runtime: runtime.kind(),
                    entrypoint: entrypoint.id().clone(),
                    kind: entrypoint.kind(),
                },
            );
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

            Self::StreamRequiresContainerRuntime(runtime) => write!(
                f,
                "stream presentation requires a container runtime, found '{runtime:?}'"
            ),
            Self::StreamEntrypointMissing(entrypoint) => write!(
                f,
                "stream presentation references missing app entrypoint '{entrypoint}'"
            ),
            Self::StreamEntrypointIncompatible(entrypoint) => write!(
                f,
                "stream presentation entrypoint '{entrypoint}' is not a container artifact"
            ),

            Self::SearchCapabilityNotProvided {
                contribution,
                capability,
            } => {
                write!(
                    f,
                    "search contribution '{contribution}' references capability '{capability}' that the app does not provide"
                )
            }
            Self::OidcClientTypeIncompatible {
                runtime,
                client_type,
            } => write!(
                f,
                "OIDC client type '{client_type:?}' is incompatible with runtime '{runtime:?}'"
            ),
            Self::OidcCallbackEntrypointMissing(entrypoint) => write!(
                f,
                "OIDC callback references missing runtime entrypoint '{entrypoint}'"
            ),
            Self::OidcCallbackEntrypointIncompatible {
                runtime,
                entrypoint,
                kind,
            } => write!(
                f,
                "OIDC callback entrypoint '{entrypoint}' of kind '{kind:?}' is incompatible with runtime '{runtime:?}'"
            ),
        }
    }
}

impl Error for AppManifestValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, CapabilityId, CommandAction, CommandContributionDeclaration,
        ContributionId, OidcCallbackPath, OidcClientDeclaration, OidcScope, PackagePath,
        PublisherId, RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint, RuntimeEntrypointId,
        SearchContributionDeclaration, StreamPresentation,
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
    fn accepts_stream_presentation_on_container_artifact_without_new_runtime_kind() {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("browser").unwrap(),
                PackagePath::parse("runtime/browser.oci").unwrap(),
            ))
            .unwrap();
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.browser").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Browser",
            runtime,
        )
        .unwrap();
        manifest
            .declare_stream_presentation(
                StreamPresentation::new(
                    RuntimeEntrypointId::parse("browser").unwrap(),
                    None,
                    Some(30),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(manifest.runtime().kind(), RuntimeKind::Container);
        assert_eq!(
            manifest
                .required_system_capabilities()
                .map(|capability| capability.to_string())
                .collect::<Vec<_>>(),
            vec!["com.rumahl.streaming.v1".to_owned()]
        );
        assert!(AppManifestValidator::new().validate(&manifest).is_ok());
    }

    #[test]
    fn rejects_stream_presentation_for_web_runtime_and_non_artifact_entrypoint() {
        let mut web = manifest();
        web.declare_stream_presentation(
            StreamPresentation::new(RuntimeEntrypointId::parse("main").unwrap(), None, None)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            AppManifestValidator::new().validate(&web),
            Err(AppManifestValidationError::StreamRequiresContainerRuntime(
                RuntimeKind::Web
            ))
        );

        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();
        let mut container = AppManifest::new(
            AppId::parse("com.rumahl.browser").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Browser",
            runtime,
        )
        .unwrap();
        container
            .declare_stream_presentation(
                StreamPresentation::new(RuntimeEntrypointId::parse("main").unwrap(), None, None)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            AppManifestValidator::new().validate(&container),
            Err(AppManifestValidationError::StreamEntrypointIncompatible(
                RuntimeEntrypointId::parse("main").unwrap()
            ))
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

    #[test]
    fn accepts_public_web_oidc_callback_bound_to_web_entrypoint() {
        let mut manifest = manifest();
        manifest
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Public,
                    RuntimeEntrypointId::parse("main").unwrap(),
                    OidcCallbackPath::parse("/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId, OidcScope::Profile],
                )
                .unwrap(),
            )
            .unwrap();

        assert!(AppManifestValidator::new().validate(&manifest).is_ok());
    }

    #[test]
    fn rejects_confidential_client_for_static_web_runtime() {
        let mut manifest = manifest();
        manifest
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Confidential,
                    RuntimeEntrypointId::parse("main").unwrap(),
                    OidcCallbackPath::parse("/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId],
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(
            AppManifestValidator::new().validate(&manifest).unwrap_err(),
            AppManifestValidationError::OidcClientTypeIncompatible {
                runtime: RuntimeKind::Web,
                client_type: OidcClientType::Confidential,
            }
        );
    }

    #[test]
    fn accepts_confidential_container_callback_bound_to_endpoint() {
        let mut runtime = RuntimeDescriptor::container();
        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();
        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.cloud").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Cloud",
            runtime,
        )
        .unwrap();
        manifest
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Confidential,
                    RuntimeEntrypointId::parse("main").unwrap(),
                    OidcCallbackPath::parse("/apps/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId, OidcScope::Profile],
                )
                .unwrap(),
            )
            .unwrap();

        assert!(AppManifestValidator::new().validate(&manifest).is_ok());
    }

    #[test]
    fn rejects_oidc_callback_for_missing_entrypoint() {
        let mut manifest = manifest();
        manifest
            .declare_oidc_client(
                OidcClientDeclaration::new(
                    OidcClientType::Public,
                    RuntimeEntrypointId::parse("missing").unwrap(),
                    OidcCallbackPath::parse("/oidc/callback").unwrap(),
                    vec![OidcScope::OpenId],
                )
                .unwrap(),
            )
            .unwrap();

        assert!(matches!(
            AppManifestValidator::new().validate(&manifest),
            Err(AppManifestValidationError::OidcCallbackEntrypointMissing(_))
        ));
    }
}
