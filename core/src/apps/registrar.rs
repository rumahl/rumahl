use std::error::Error;
use std::fmt;

use crate::{
    CapabilityRegistryError, CommandRegistryError, ContributionRegistryError, EventBusError,
    PlatformState, SearchRegistryError,
};

use super::PlatformRegistration;

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformRegistrar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformRegistrarError {
    CapabilityConflict(CapabilityRegistryError),
    ContributionConflict(ContributionRegistryError),
    CommandConflict(CommandRegistryError),
    SearchConflict(SearchRegistryError),
    EventSubscriptionConflict(EventBusError),
}

impl PlatformRegistrar {
    pub fn new() -> Self {
        Self
    }

    pub fn can_register(
        &self,
        registration: &PlatformRegistration,
        state: &PlatformState,
    ) -> Result<(), PlatformRegistrarError> {
        for provider in registration.capability_providers() {
            state
                .capability_registry()
                .can_register(provider)
                .map_err(PlatformRegistrarError::CapabilityConflict)?;
        }

        for contribution in registration.contributions() {
            state
                .contribution_registry()
                .can_register(contribution)
                .map_err(PlatformRegistrarError::ContributionConflict)?;
        }

        for command in registration.commands() {
            state
                .command_registry()
                .can_register(command)
                .map_err(PlatformRegistrarError::CommandConflict)?;
        }

        for search in registration.searches() {
            state
                .search_registry()
                .can_register(search)
                .map_err(PlatformRegistrarError::SearchConflict)?;
        }

        for subscription in registration.event_subscriptions() {
            state
                .event_bus()
                .can_subscribe(subscription)
                .map_err(PlatformRegistrarError::EventSubscriptionConflict)?;
        }

        Ok(())
    }

    pub fn register(
        &self,
        registration: &PlatformRegistration,
        state: &mut PlatformState,
    ) -> Result<(), PlatformRegistrarError> {
        self.can_register(registration, state)?;

        let mut staged = state.clone();

        for provider in registration.capability_providers() {
            staged
                .capability_registry_mut()
                .register(provider.clone())
                .map_err(PlatformRegistrarError::CapabilityConflict)?;
        }

        for contribution in registration.contributions() {
            staged
                .contribution_registry_mut()
                .register(contribution.clone())
                .map_err(PlatformRegistrarError::ContributionConflict)?;
        }

        for command in registration.commands() {
            staged
                .command_registry_mut()
                .register(command.clone())
                .map_err(PlatformRegistrarError::CommandConflict)?;
        }

        for search in registration.searches() {
            staged
                .search_registry_mut()
                .register(search.clone())
                .map_err(PlatformRegistrarError::SearchConflict)?;
        }

        for subscription in registration.event_subscriptions() {
            staged
                .event_bus_mut()
                .subscribe(subscription.clone())
                .map_err(PlatformRegistrarError::EventSubscriptionConflict)?;
        }

        *state = staged;

        Ok(())
    }
}

impl fmt::Display for PlatformRegistrarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityConflict(error) => {
                write!(f, "capability registration conflict: {error}")
            }

            Self::ContributionConflict(error) => {
                write!(f, "contribution registration conflict: {error}")
            }

            Self::CommandConflict(error) => {
                write!(f, "command registration conflict: {error}")
            }

            Self::SearchConflict(error) => {
                write!(f, "search registration conflict: {error}")
            }

            Self::EventSubscriptionConflict(error) => {
                write!(f, "event subscription conflict: {error}")
            }
        }
    }
}

impl Error for PlatformRegistrarError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CapabilityConflict(error) => Some(error),

            Self::ContributionConflict(error) => Some(error),

            Self::CommandConflict(error) => Some(error),

            Self::SearchConflict(error) => Some(error),

            Self::EventSubscriptionConflict(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, CapabilityId, CommandAction,
        CommandContributionDeclaration, ContributionId, EventName, InstalledApp, PublisherId,
        SearchContributionDeclaration,
    };

    fn registration() -> PlatformRegistration {
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

        let app = InstalledApp::install(manifest, &AppManifestValidator::new()).unwrap();

        PlatformRegistration::prepare(&app).unwrap()
    }

    #[test]
    fn accepts_registration_without_mutating_platform() {
        let registration = registration();
        let state = PlatformState::new();

        let registrar = PlatformRegistrar::new();

        registrar.can_register(&registration, &state).unwrap();

        assert!(state.capability_registry().is_empty());
        assert!(state.contribution_registry().is_empty());
        assert!(state.command_registry().is_empty());
        assert!(state.search_registry().is_empty());
        assert!(state.event_bus().is_empty());
    }

    #[test]
    fn detects_capability_conflict() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .capability_registry_mut()
            .register(registration.capability_providers()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(&registration, &state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::CapabilityConflict(CapabilityRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_contribution_conflict() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .contribution_registry_mut()
            .register(registration.contributions()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(&registration, &state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::ContributionConflict(
                ContributionRegistryError::AlreadyRegistered
            )
        );
    }

    #[test]
    fn detects_command_conflict() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .command_registry_mut()
            .register(registration.commands()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(&registration, &state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::CommandConflict(CommandRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_search_conflict() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .search_registry_mut()
            .register(registration.searches()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(&registration, &state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::SearchConflict(SearchRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_event_subscription_conflict() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .event_bus_mut()
            .subscribe(registration.event_subscriptions()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(&registration, &state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::EventSubscriptionConflict(EventBusError::AlreadySubscribed)
        );
    }

    #[test]
    fn registers_platform_registration_atomically() {
        let registration = registration();

        let mut state = PlatformState::new();

        let registrar = PlatformRegistrar::new();

        registrar.register(&registration, &mut state).unwrap();

        assert_eq!(
            state.capability_registry().len(),
            registration.capability_providers().len()
        );

        assert_eq!(
            state.contribution_registry().len(),
            registration.contributions().len()
        );

        assert_eq!(
            state.command_registry().len(),
            registration.commands().len()
        );

        assert_eq!(state.search_registry().len(), registration.searches().len());

        assert_eq!(
            state.event_bus().len(),
            registration.event_subscriptions().len()
        );
    }

    #[test]
    fn registration_conflict_leaves_platform_unchanged() {
        let registration = registration();

        let mut state = PlatformState::new();

        state
            .event_bus_mut()
            .subscribe(registration.event_subscriptions()[0].clone())
            .unwrap();

        let capabilities_before = state.capability_registry().len();

        let contributions_before = state.contribution_registry().len();

        let commands_before = state.command_registry().len();

        let searches_before = state.search_registry().len();

        let subscriptions_before = state.event_bus().len();

        let registrar = PlatformRegistrar::new();

        let result = registrar.register(&registration, &mut state);

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::EventSubscriptionConflict(EventBusError::AlreadySubscribed)
        );

        assert_eq!(state.capability_registry().len(), capabilities_before);

        assert_eq!(state.contribution_registry().len(), contributions_before);

        assert_eq!(state.command_registry().len(), commands_before);

        assert_eq!(state.search_registry().len(), searches_before);

        assert_eq!(state.event_bus().len(), subscriptions_before);
    }
}
