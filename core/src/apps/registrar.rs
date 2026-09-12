use std::error::Error;
use std::fmt;

use crate::{
    CapabilityRegistry, CapabilityRegistryError, CommandRegistry, CommandRegistryError,
    ContributionRegistry, ContributionRegistryError, EventBus, EventBusError, SearchRegistry,
    SearchRegistryError,
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
        capability_registry: &CapabilityRegistry,
        contribution_registry: &ContributionRegistry,
        command_registry: &CommandRegistry,
        search_registry: &SearchRegistry,
        event_bus: &EventBus,
    ) -> Result<(), PlatformRegistrarError> {
        for provider in registration.capability_providers() {
            capability_registry
                .can_register(provider)
                .map_err(PlatformRegistrarError::CapabilityConflict)?;
        }

        for contribution in registration.contributions() {
            contribution_registry
                .can_register(contribution)
                .map_err(PlatformRegistrarError::ContributionConflict)?;
        }

        for command in registration.commands() {
            command_registry
                .can_register(command)
                .map_err(PlatformRegistrarError::CommandConflict)?;
        }

        for search in registration.searches() {
            search_registry
                .can_register(search)
                .map_err(PlatformRegistrarError::SearchConflict)?;
        }

        for subscription in registration.event_subscriptions() {
            event_bus
                .can_subscribe(subscription)
                .map_err(PlatformRegistrarError::EventSubscriptionConflict)?;
        }

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

        let capability_registry = CapabilityRegistry::new();

        let contribution_registry = ContributionRegistry::new();

        let command_registry = CommandRegistry::new();

        let search_registry = SearchRegistry::new();

        let event_bus = EventBus::new();

        let registrar = PlatformRegistrar::new();

        registrar
            .can_register(
                &registration,
                &capability_registry,
                &contribution_registry,
                &command_registry,
                &search_registry,
                &event_bus,
            )
            .unwrap();

        assert!(capability_registry.is_empty());

        assert!(contribution_registry.is_empty());

        assert!(command_registry.is_empty());

        assert!(search_registry.is_empty());

        assert!(event_bus.is_empty());
    }

    #[test]
    fn detects_capability_conflict() {
        let registration = registration();

        let mut capability_registry = CapabilityRegistry::new();

        capability_registry
            .register(registration.capability_providers()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(
            &registration,
            &capability_registry,
            &ContributionRegistry::new(),
            &CommandRegistry::new(),
            &SearchRegistry::new(),
            &EventBus::new(),
        );

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::CapabilityConflict(CapabilityRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_contribution_conflict() {
        let registration = registration();

        let mut contribution_registry = ContributionRegistry::new();

        contribution_registry
            .register(registration.contributions()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(
            &registration,
            &CapabilityRegistry::new(),
            &contribution_registry,
            &CommandRegistry::new(),
            &SearchRegistry::new(),
            &EventBus::new(),
        );

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

        let mut command_registry = CommandRegistry::new();

        command_registry
            .register(registration.commands()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(
            &registration,
            &CapabilityRegistry::new(),
            &ContributionRegistry::new(),
            &command_registry,
            &SearchRegistry::new(),
            &EventBus::new(),
        );

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::CommandConflict(CommandRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_search_conflict() {
        let registration = registration();

        let mut search_registry = SearchRegistry::new();

        search_registry
            .register(registration.searches()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(
            &registration,
            &CapabilityRegistry::new(),
            &ContributionRegistry::new(),
            &CommandRegistry::new(),
            &search_registry,
            &EventBus::new(),
        );

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::SearchConflict(SearchRegistryError::AlreadyRegistered)
        );
    }

    #[test]
    fn detects_event_subscription_conflict() {
        let registration = registration();

        let mut event_bus = EventBus::new();

        event_bus
            .subscribe(registration.event_subscriptions()[0].clone())
            .unwrap();

        let registrar = PlatformRegistrar::new();

        let result = registrar.can_register(
            &registration,
            &CapabilityRegistry::new(),
            &ContributionRegistry::new(),
            &CommandRegistry::new(),
            &SearchRegistry::new(),
            &event_bus,
        );

        assert_eq!(
            result.unwrap_err(),
            PlatformRegistrarError::EventSubscriptionConflict(EventBusError::AlreadySubscribed)
        );
    }
}
