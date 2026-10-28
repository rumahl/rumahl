use crate::Identity;

use super::{EventEnvelope, EventId, EventName};

#[derive(Debug, Clone)]
pub struct EventDelivery {
    subscriber: Identity,
    event: EventEnvelope,
}

impl EventDelivery {
    pub(crate) fn new(subscriber: Identity, event: EventEnvelope) -> Self {
        Self { subscriber, event }
    }

    pub fn subscriber(&self) -> &Identity {
        &self.subscriber
    }

    pub fn event(&self) -> &EventEnvelope {
        &self.event
    }

    pub fn event_id(&self) -> EventId {
        self.event.id()
    }

    pub fn event_name(&self) -> &EventName {
        self.event.name()
    }
}
