use std::error::Error;
use std::fmt;

use crate::{InstallationId, PlatformState};

use super::{
    AppManifest, AppManifestValidator, InstalledApp, InstalledAppError, InstalledAppRegistryError,
    PlatformDeregistrationReport, PlatformRegistrar, PlatformRegistrarError, PlatformRegistration,
    PlatformRegistrationError,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct AppLifecycle {
    validator: AppManifestValidator,
    registrar: PlatformRegistrar,
}

#[derive(Debug, Clone)]
pub struct AppUninstallResult {
    app: InstalledApp,
    deregistration: PlatformDeregistrationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppLifecycleError {
    InvalidApp(InstalledAppError),
    RegistrationPreparationFailed(PlatformRegistrationError),
    InstalledAppConflict(InstalledAppRegistryError),
    PlatformRegistrationConflict(PlatformRegistrarError),
    InstallationNotFound,
}

impl AppLifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install(
        &self,
        manifest: AppManifest,
        state: &mut PlatformState,
    ) -> Result<InstalledApp, AppLifecycleError> {
        /*
         * Build the concrete installation.
         */

        let app = InstalledApp::create(manifest, &self.validator)
            .map_err(AppLifecycleError::InvalidApp)?;

        /*
         * Build the complete platform
         * registration before mutating state.
         */

        let registration = PlatformRegistration::prepare(&app)
            .map_err(AppLifecycleError::RegistrationPreparationFailed)?;

        /*
         * Complete preflight.
         */

        state
            .installed_apps()
            .can_register(&app)
            .map_err(AppLifecycleError::InstalledAppConflict)?;

        self.registrar
            .can_register(&registration, state)
            .map_err(AppLifecycleError::PlatformRegistrationConflict)?;

        /*
         * Everything after this point happens
         * against a staged copy.
         */

        let mut staged = state.clone();

        staged
            .installed_apps_mut()
            .register(app.clone())
            .map_err(AppLifecycleError::InstalledAppConflict)?;

        self.registrar
            .apply_registration(&registration, &mut staged)
            .map_err(AppLifecycleError::PlatformRegistrationConflict)?;

        /*
         * Atomic commit.
         */

        *state = staged;

        Ok(app)
    }

    pub fn uninstall(
        &self,
        installation_id: &InstallationId,
        state: &mut PlatformState,
    ) -> Result<AppUninstallResult, AppLifecycleError> {
        /*
         * Resolve and clone before taking mutable
         * access to PlatformState.
         */

        let app = state
            .installed_apps()
            .get_by_installation_id(installation_id)
            .cloned()
            .ok_or(AppLifecycleError::InstallationNotFound)?;

        let mut staged = state.clone();

        let deregistration = self.registrar.apply_deregistration(&app, &mut staged);

        let removed = staged
            .installed_apps_mut()
            .remove(installation_id)
            .ok_or(AppLifecycleError::InstallationNotFound)?;

        /*
         * Only now replace the live state.
         */

        *state = staged;

        Ok(AppUninstallResult {
            app: removed,
            deregistration,
        })
    }
}

impl AppUninstallResult {
    pub fn app(&self) -> &InstalledApp {
        &self.app
    }

    pub fn deregistration(&self) -> &PlatformDeregistrationReport {
        &self.deregistration
    }
}

impl fmt::Display for AppLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidApp(error) => {
                write!(f, "cannot install app: {error}")
            }

            Self::RegistrationPreparationFailed(error) => {
                write!(f, "cannot prepare app registration: {error}")
            }

            Self::InstalledAppConflict(error) => {
                write!(f, "installed app conflict: {error}")
            }

            Self::PlatformRegistrationConflict(error) => {
                write!(f, "platform registration conflict: {error}")
            }

            Self::InstallationNotFound => {
                write!(f, "app installation was not found")
            }
        }
    }
}

impl Error for AppLifecycleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidApp(error) => Some(error),

            Self::RegistrationPreparationFailed(error) => Some(error),

            Self::InstalledAppConflict(error) => Some(error),

            Self::PlatformRegistrationConflict(error) => Some(error),

            Self::InstallationNotFound => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, CapabilityId, CommandAction, CommandContributionDeclaration,
        Contribution, ContributionId, ContributionKind, EventName, PublisherId,
        SearchContributionDeclaration,
    };

    fn manifest() -> AppManifest {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        manifest
            .add_provided_capability(CapabilityId::parse("rumahl.search.query").unwrap())
            .unwrap();

        manifest
            .add_contribution(
                CommandContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.open").unwrap(),
                    "Open Notes",
                    CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
                )
                .unwrap()
                .into(),
            )
            .unwrap();

        manifest
            .add_contribution(
                SearchContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.search").unwrap(),
                    CapabilityId::parse("rumahl.search.query").unwrap(),
                )
                .into(),
            )
            .unwrap();

        manifest
            .add_event_subscription(EventName::parse("rumahl.files.changed").unwrap())
            .unwrap();

        manifest
    }

    #[test]
    fn installs_app_and_platform_registration() {
        let lifecycle = AppLifecycle::new();

        let mut state = PlatformState::new();

        let app = lifecycle.install(manifest(), &mut state).unwrap();

        assert_eq!(state.installed_apps().len(), 1);

        assert_eq!(state.capability_registry().len(), 1);

        assert_eq!(state.contribution_registry().len(), 2);

        assert_eq!(state.command_registry().len(), 1);

        assert_eq!(state.search_registry().len(), 1);

        assert_eq!(state.event_bus().len(), 1);

        assert!(
            state
                .installed_apps()
                .get_by_installation_id(app.installation_id())
                .is_some()
        );
    }

    #[test]
    fn rejects_second_installation_of_same_app_without_mutating_state() {
        let lifecycle = AppLifecycle::new();

        let mut state = PlatformState::new();

        lifecycle.install(manifest(), &mut state).unwrap();

        let installed_before = state.installed_apps().len();

        let capabilities_before = state.capability_registry().len();

        let contributions_before = state.contribution_registry().len();

        let commands_before = state.command_registry().len();

        let searches_before = state.search_registry().len();

        let subscriptions_before = state.event_bus().len();

        let result = lifecycle.install(manifest(), &mut state);

        assert_eq!(
            result.unwrap_err(),
            AppLifecycleError::InstalledAppConflict(InstalledAppRegistryError::AppAlreadyInstalled)
        );

        assert_eq!(state.installed_apps().len(), installed_before);

        assert_eq!(state.capability_registry().len(), capabilities_before);

        assert_eq!(state.contribution_registry().len(), contributions_before);

        assert_eq!(state.command_registry().len(), commands_before);

        assert_eq!(state.search_registry().len(), searches_before);

        assert_eq!(state.event_bus().len(), subscriptions_before);
    }

    #[test]
    fn platform_conflict_does_not_register_installed_app() {
        let lifecycle = AppLifecycle::new();

        let mut state = PlatformState::new();

        let other_app = InstalledApp::create(
            AppManifest::new(
                AppId::parse("com.rumahl.other").unwrap(),
                PublisherId::parse("com.rumahl").unwrap(),
                AppVersion::new(1, 0, 0),
                "Other",
            )
            .unwrap(),
            &AppManifestValidator::new(),
        )
        .unwrap();

        state
            .contribution_registry_mut()
            .register(
                Contribution::new(
                    ContributionId::parse("com.rumahl.notes.open").unwrap(),
                    other_app.identity().clone().into(),
                    ContributionKind::parse("command").unwrap(),
                )
                .unwrap(),
            )
            .unwrap();

        let result = lifecycle.install(manifest(), &mut state);

        assert!(matches!(
            result,
            Err(AppLifecycleError::PlatformRegistrationConflict(
                PlatformRegistrarError::ContributionConflict(_)
            ))
        ));

        assert!(state.installed_apps().is_empty());

        /*
         * Only the contribution that existed
         * before the attempted installation
         * remains.
         */

        assert_eq!(state.contribution_registry().len(), 1);

        assert!(state.capability_registry().is_empty());

        assert!(state.command_registry().is_empty());

        assert!(state.search_registry().is_empty());

        assert!(state.event_bus().is_empty());
    }

    #[test]
    fn uninstalls_app_and_all_platform_registration() {
        let lifecycle = AppLifecycle::new();

        let mut state = PlatformState::new();

        let app = lifecycle.install(manifest(), &mut state).unwrap();

        let installation_id = *app.installation_id();

        let result = lifecycle.uninstall(&installation_id, &mut state).unwrap();

        assert_eq!(result.app().installation_id(), &installation_id);

        assert!(!result.deregistration().is_empty());

        assert!(state.installed_apps().is_empty());

        assert!(state.capability_registry().is_empty());

        assert!(state.contribution_registry().is_empty());

        assert!(state.command_registry().is_empty());

        assert!(state.search_registry().is_empty());

        assert!(state.event_bus().is_empty());
    }

    #[test]
    fn rejects_uninstall_of_unknown_installation() {
        let lifecycle = AppLifecycle::new();

        let mut state = PlatformState::new();

        let installation_id = InstallationId::new();

        let result = lifecycle.uninstall(&installation_id, &mut state);

        assert!(matches!(
            result,
            Err(AppLifecycleError::InstallationNotFound)
        ));

        assert!(state.installed_apps().is_empty());
    }
}
