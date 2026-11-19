//! Builds the runtime capability registries from the installed app set.
//!
//! Capability *providers* come straight from each installed app's manifest
//! (`provided_capabilities`). The *access rule* for a capability is a platform
//! policy: a consumer must hold the capability's own identifier as a
//! permission. Grants are issued separately, so an app without a grant is
//! denied by default.
use std::error::Error;
use std::fmt;

use crate::{PermissionId, PlatformRegistration, PlatformRegistrationError, PlatformState};

use super::{
    CapabilityAccessRegistry, CapabilityAccessRegistryError, CapabilityAccessRule,
    CapabilityRegistry, CapabilityRegistryError,
};

#[derive(Debug)]
pub enum CapabilityRegistryBuildError {
    Registration(PlatformRegistrationError),
    Registry(CapabilityRegistryError),
    Access(CapabilityAccessRegistryError),
}

pub fn build_capability_registries(
    state: &PlatformState,
) -> Result<(CapabilityRegistry, CapabilityAccessRegistry), CapabilityRegistryBuildError> {
    let mut capabilities = CapabilityRegistry::new();
    let mut access = CapabilityAccessRegistry::new();

    for app in state.installed_apps().apps() {
        let registration = PlatformRegistration::prepare(app)
            .map_err(CapabilityRegistryBuildError::Registration)?;

        for provider in registration.capability_providers() {
            let capability = provider.capability().clone();

            if !capabilities.is_registered(provider) {
                capabilities
                    .register(provider.clone())
                    .map_err(CapabilityRegistryBuildError::Registry)?;
            }

            if access.rule_for(&capability).is_none()
                && let Ok(permission) = PermissionId::parse(capability.as_str())
            {
                access
                    .register(CapabilityAccessRule::new(capability, permission))
                    .map_err(CapabilityRegistryBuildError::Access)?;
            }
        }
    }

    Ok((capabilities, access))
}

impl fmt::Display for CapabilityRegistryBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registration(error) => write!(f, "app registration failed: {error}"),
            Self::Registry(error) => write!(f, "capability registry failed: {error}"),
            Self::Access(error) => write!(f, "capability access registry failed: {error}"),
        }
    }
}

impl Error for CapabilityRegistryBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registration(error) => Some(error),
            Self::Registry(error) => Some(error),
            Self::Access(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, CapabilityId, InstalledApp,
        PackagePath, PlatformState, PublisherId, RuntimeDescriptor, RuntimeEntrypoint,
        RuntimeEntrypointId,
    };

    fn app(id: &str, capability: &str) -> InstalledApp {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        let mut manifest = AppManifest::new(
            AppId::parse(id).unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "App",
            runtime,
        )
        .unwrap();

        manifest
            .add_provided_capability(CapabilityId::parse(capability).unwrap())
            .unwrap();

        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn derives_providers_and_access_rules_from_installed_apps() {
        let mut state = PlatformState::new();
        state
            .installed_apps_mut()
            .register(app("com.rumahl.files", "rumahl.files.preview"))
            .unwrap();

        let (capabilities, access) = build_capability_registries(&state).unwrap();

        let capability = CapabilityId::parse("rumahl.files.preview").unwrap();
        assert_eq!(capabilities.providers_for(&capability).len(), 1);
        let rule = access.rule_for(&capability).unwrap();
        assert_eq!(rule.permission().as_str(), "rumahl.files.preview");
    }
}
