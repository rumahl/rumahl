use std::error::Error;
use std::fmt;

use crate::{
    CapabilityProvider, CapabilityProviderError, CommandContribution, CommandContributionError,
    Contribution, ContributionError, EventSubscription, EventSubscriptionError, SearchContribution,
};

use super::{ContributionDeclaration, InstalledApp};

#[derive(Debug, Clone)]
pub struct PlatformRegistration {
    capability_providers: Vec<CapabilityProvider>,
    contributions: Vec<Contribution>,
    commands: Vec<CommandContribution>,
    searches: Vec<SearchContribution>,
    event_subscriptions: Vec<EventSubscription>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformRegistrationError {
    InvalidCapabilityProvider(CapabilityProviderError),
    InvalidCommandContribution(CommandContributionError),
    InvalidSearchContribution(ContributionError),
    InvalidEventSubscription(EventSubscriptionError),
}

impl PlatformRegistration {
    pub fn prepare(app: &InstalledApp) -> Result<Self, PlatformRegistrationError> {
        let identity = app.identity().clone();

        let manifest = app.manifest();

        let mut capability_providers = Vec::new();

        let mut contributions = Vec::new();

        let mut commands = Vec::new();

        let mut searches = Vec::new();

        let mut event_subscriptions = Vec::new();

        /*
         * Provided capabilities.
         */

        for capability in manifest.provided_capabilities() {
            let provider = CapabilityProvider::new(identity.clone().into(), capability.clone())
                .map_err(PlatformRegistrationError::InvalidCapabilityProvider)?;

            capability_providers.push(provider);
        }

        /*
         * Contributions.
         */

        for declaration in manifest.contributions() {
            match declaration {
                ContributionDeclaration::Command(declaration) => {
                    let command = CommandContribution::new(
                        declaration.id().clone(),
                        identity.clone().into(),
                        declaration.title(),
                        declaration.action().clone(),
                    )
                    .map_err(PlatformRegistrationError::InvalidCommandContribution)?;

                    contributions.push(command.contribution().clone());

                    commands.push(command);
                }

                ContributionDeclaration::Search(declaration) => {
                    let search = SearchContribution::new(
                        declaration.id().clone(),
                        identity.clone().into(),
                        declaration.capability().clone(),
                    )
                    .map_err(PlatformRegistrationError::InvalidSearchContribution)?;

                    contributions.push(search.contribution().clone());

                    searches.push(search);
                }
            }
        }

        /*
         * Event subscriptions.
         */

        for event in manifest.event_subscriptions() {
            let subscription = EventSubscription::new(identity.clone().into(), event.clone())
                .map_err(PlatformRegistrationError::InvalidEventSubscription)?;

            event_subscriptions.push(subscription);
        }

        Ok(Self {
            capability_providers,
            contributions,
            commands,
            searches,
            event_subscriptions,
        })
    }

    pub fn capability_providers(&self) -> &[CapabilityProvider] {
        &self.capability_providers
    }

    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }

    pub fn commands(&self) -> &[CommandContribution] {
        &self.commands
    }

    pub fn searches(&self) -> &[SearchContribution] {
        &self.searches
    }

    pub fn event_subscriptions(&self) -> &[EventSubscription] {
        &self.event_subscriptions
    }
}

impl fmt::Display for PlatformRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapabilityProvider(error) => {
                write!(f, "cannot prepare capability provider: {error}")
            }

            Self::InvalidCommandContribution(error) => {
                write!(f, "cannot prepare command contribution: {error}")
            }

            Self::InvalidSearchContribution(error) => {
                write!(f, "cannot prepare search contribution: {error}")
            }

            Self::InvalidEventSubscription(error) => {
                write!(f, "cannot prepare event subscription: {error}")
            }
        }
    }
}

impl Error for PlatformRegistrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCapabilityProvider(error) => Some(error),

            Self::InvalidCommandContribution(error) => Some(error),

            Self::InvalidSearchContribution(error) => Some(error),

            Self::InvalidEventSubscription(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppManifest, AppManifestValidator, AppVersion, CapabilityId, CommandAction,
        CommandContributionDeclaration, ContributionId, EventName, PublisherId,
        SearchContributionDeclaration,
    };

    fn installed_app() -> InstalledApp {
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
            .add_provided_capability(CapabilityId::parse("com.rumahl.notes.create-note").unwrap())
            .unwrap();

        manifest
            .add_contribution(
                CommandContributionDeclaration::new(
                    ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
                    "New note",
                    CommandAction::invoke_capability(
                        CapabilityId::parse("com.rumahl.notes.create-note").unwrap(),
                    ),
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

        InstalledApp::install(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn prepares_platform_registration() {
        let app = installed_app();

        let registration = PlatformRegistration::prepare(&app).unwrap();

        assert_eq!(registration.capability_providers().len(), 2);

        assert_eq!(registration.contributions().len(), 2);

        assert_eq!(registration.commands().len(), 1);

        assert_eq!(registration.searches().len(), 1);

        assert_eq!(registration.event_subscriptions().len(), 1);
    }

    #[test]
    fn registration_uses_installed_app_identity() {
        let app = installed_app();

        let identity = app.identity().clone().into();

        let registration = PlatformRegistration::prepare(&app).unwrap();

        assert!(
            registration
                .capability_providers()
                .iter()
                .all(|provider| { provider.identity() == &identity })
        );

        assert!(
            registration
                .commands()
                .iter()
                .all(|command| { command.owner() == &identity })
        );

        assert!(
            registration
                .searches()
                .iter()
                .all(|search| { search.owner() == &identity })
        );

        assert!(
            registration
                .event_subscriptions()
                .iter()
                .all(|subscription| { subscription.subscriber() == &identity })
        );
    }

    #[test]
    fn search_registration_resolves_to_prepared_provider() {
        let app = installed_app();

        let registration = PlatformRegistration::prepare(&app).unwrap();

        let search = &registration.searches()[0];

        let matching_provider = registration.capability_providers().iter().find(|provider| {
            provider.identity() == search.owner() && provider.capability() == search.capability()
        });

        assert!(matching_provider.is_some());
    }
}
