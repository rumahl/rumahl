use std::error::Error;
use std::fmt;

use super::{EventDelivery, EventEnvelope, EventSubscription};

#[derive(Debug, Default)]
pub struct EventBus {
    subscriptions: Vec<EventSubscription>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBusError {
    AlreadySubscribed,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_subscribe(&self, subscription: &EventSubscription) -> Result<(), EventBusError> {
        if self
            .subscriptions
            .iter()
            .any(|existing| existing == subscription)
        {
            return Err(EventBusError::AlreadySubscribed);
        }

        Ok(())
    }

    pub fn subscribe(&mut self, subscription: EventSubscription) -> Result<(), EventBusError> {
        self.can_subscribe(&subscription)?;

        self.subscriptions.push(subscription);

        Ok(())
    }

    pub fn prepare_deliveries(&self, event: &EventEnvelope) -> Vec<EventDelivery> {
        self.subscriptions
            .iter()
            .filter(|subscription| subscription.event() == event.name())
            .map(|subscription| {
                EventDelivery::new(subscription.subscriber().clone(), event.clone())
            })
            .collect()
    }

    pub fn subscriptions(&self) -> &[EventSubscription] {
        &self.subscriptions
    }

    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

impl fmt::Display for EventBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadySubscribed => {
                write!(f, "subscriber is already subscribed to this event")
            }
        }
    }
}

impl Error for EventBusError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, OperationContext, PublisherId};

    use super::super::{EventName, EventSubscription};

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn registers_subscription() {
        let mut bus = EventBus::new();

        bus.subscribe(
            EventSubscription::new(
                app("com.rumahl.notes").into(),
                EventName::parse("rumahl.files.changed").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(bus.len(), 1);

        assert!(!bus.is_empty());
    }

    #[test]
    fn rejects_duplicate_subscription() {
        let notes = app("com.rumahl.notes");

        let event = EventName::parse("rumahl.files.changed").unwrap();

        let first = EventSubscription::new(notes.clone().into(), event.clone()).unwrap();

        let duplicate = EventSubscription::new(notes.into(), event).unwrap();

        let mut bus = EventBus::new();

        bus.subscribe(first).unwrap();

        assert_eq!(
            bus.subscribe(duplicate).unwrap_err(),
            EventBusError::AlreadySubscribed
        );
    }

    #[test]
    fn prepares_delivery_for_matching_subscription() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let notes_identity = notes.clone().into();

        let event_name = EventName::parse("rumahl.files.changed").unwrap();

        let mut bus = EventBus::new();

        bus.subscribe(EventSubscription::new(notes.into(), event_name.clone()).unwrap())
            .unwrap();

        let event = EventEnvelope::new(
            event_name,
            OperationContext::for_background_app(files),
            None,
        );

        let deliveries = bus.prepare_deliveries(&event);

        assert_eq!(deliveries.len(), 1);

        assert_eq!(deliveries[0].subscriber(), &notes_identity);

        assert_eq!(deliveries[0].event_id(), event.id());
    }

    #[test]
    fn ignores_unrelated_subscription() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let mut bus = EventBus::new();

        bus.subscribe(
            EventSubscription::new(
                notes.into(),
                EventName::parse("rumahl.apps.installed").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        let event = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(files),
            None,
        );

        let deliveries = bus.prepare_deliveries(&event);

        assert!(deliveries.is_empty());
    }

    //multi subscription

    #[test]
    fn prepares_delivery_for_each_subscriber() {
        let notes = app("com.rumahl.notes");

        let search = app("com.rumahl.search");

        let files = app("com.rumahl.files");

        let event_name = EventName::parse("rumahl.files.changed").unwrap();

        let mut bus = EventBus::new();

        bus.subscribe(EventSubscription::new(notes.into(), event_name.clone()).unwrap())
            .unwrap();

        bus.subscribe(EventSubscription::new(search.into(), event_name.clone()).unwrap())
            .unwrap();

        let event = EventEnvelope::new(
            event_name,
            OperationContext::for_background_app(files),
            None,
        );

        let deliveries = bus.prepare_deliveries(&event);

        assert_eq!(deliveries.len(), 2);

        assert!(
            deliveries
                .iter()
                .all(|delivery| { delivery.event_id() == event.id() })
        );
    }

    #[test]
    fn can_subscribe_without_mutating_bus() {
        let subscription = EventSubscription::new(
            app("com.rumahl.notes").into(),
            EventName::parse("rumahl.files.changed").unwrap(),
        )
        .unwrap();

        let bus = EventBus::new();

        assert!(bus.can_subscribe(&subscription).is_ok());

        assert!(bus.is_empty());
    }

    #[test]
    fn cannot_subscribe_duplicate() {
        let subscription = EventSubscription::new(
            app("com.rumahl.notes").into(),
            EventName::parse("rumahl.files.changed").unwrap(),
        )
        .unwrap();

        let mut bus = EventBus::new();

        bus.subscribe(subscription.clone()).unwrap();

        assert_eq!(
            bus.can_subscribe(&subscription).unwrap_err(),
            EventBusError::AlreadySubscribed
        );
    }
}
