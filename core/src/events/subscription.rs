use std::error::Error;
use std::fmt;

use crate::Identity;

use super::EventName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSubscription {
    subscriber: Identity,
    event: EventName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSubscriptionError {
    UserCannotSubscribe,
}

impl EventSubscription {
    pub fn new(subscriber: Identity, event: EventName) -> Result<Self, EventSubscriptionError> {
        if subscriber.is_user() {
            return Err(EventSubscriptionError::UserCannotSubscribe);
        }

        Ok(Self { subscriber, event })
    }

    pub fn subscriber(&self) -> &Identity {
        &self.subscriber
    }

    pub fn event(&self) -> &EventName {
        &self.event
    }
}

impl fmt::Display for EventSubscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserCannotSubscribe => {
                write!(f, "users cannot subscribe directly to platform events")
            }
        }
    }
}

impl Error for EventSubscriptionError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, PublisherId, UserId, UserIdentity};

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn app_can_subscribe() {
        let subscription = EventSubscription::new(
            notes_app().into(),
            EventName::parse("rumahl.files.changed").unwrap(),
        );

        assert!(subscription.is_ok());
    }

    #[test]
    fn user_cannot_subscribe_directly() {
        let user = UserIdentity::new(UserId::new());

        let result = EventSubscription::new(
            user.into(),
            EventName::parse("rumahl.files.changed").unwrap(),
        );

        assert_eq!(
            result.unwrap_err(),
            EventSubscriptionError::UserCannotSubscribe
        );
    }
}
