//! Live channels to running app instances.
//!
//! The window bridge (`postMessage`) only exists while an iframe is open.
//! Always-on services keep a persistent, authenticated channel open while they
//! run without a window (a loopback socket or pipe), so OS events still reach
//! them. This module models the OS side: a per-installation registry of live
//! channels and the event envelope the OS pushes.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{EventDelivery, Identity, InstallationId};

/// A message the OS pushes to a running app instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvent {
    pub topic: String,
    /// The addressed resource (`namespace/kind/key`) when the event carries one.
    pub resource: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeChannelError {
    /// No live channel is registered for this installation.
    Unavailable,
    /// A channel exists but rejected the message.
    Rejected,
}

/// A live, authenticated channel to a running app instance.
pub trait RuntimeChannel: Send + Sync {
    fn deliver(&self, event: &RuntimeEvent) -> Result<(), RuntimeChannelError>;
}

/// Installation-scoped registry of live app channels.
#[derive(Default)]
pub struct RuntimeChannelRegistry {
    channels: Mutex<HashMap<InstallationId, Arc<dyn RuntimeChannel>>>,
}

impl RuntimeChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a channel for an installation. Returns `false` when a channel
    /// is already live (the caller must not replace a running channel blindly).
    pub fn register(
        &self,
        installation: InstallationId,
        channel: Arc<dyn RuntimeChannel>,
    ) -> bool {
        match self.channels.lock() {
            Ok(mut channels) => {
                if channels.contains_key(&installation) {
                    return false;
                }
                channels.insert(installation, channel);
                true
            }
            Err(_) => false,
        }
    }

    pub fn unregister(&self, installation: &InstallationId) -> bool {
        self.channels
            .lock()
            .map(|mut channels| channels.remove(installation).is_some())
            .unwrap_or(false)
    }

    pub fn is_registered(&self, installation: &InstallationId) -> bool {
        self.channels
            .lock()
            .map(|channels| channels.contains_key(installation))
            .unwrap_or(false)
    }

    pub fn deliver(
        &self,
        installation: &InstallationId,
        event: &RuntimeEvent,
    ) -> Result<(), RuntimeChannelError> {
        let channel = self
            .channels
            .lock()
            .map_err(|_| RuntimeChannelError::Unavailable)?
            .get(installation)
            .cloned()
            .ok_or(RuntimeChannelError::Unavailable)?;
        channel.deliver(event)
    }

    /// Delivers an OS event to the subscribed app's live channel.
    pub fn deliver_event(&self, delivery: &EventDelivery) -> Result<(), RuntimeChannelError> {
        let Identity::App(identity) = delivery.subscriber() else {
            return Err(RuntimeChannelError::Rejected);
        };
        let event = RuntimeEvent {
            topic: delivery.event().name().as_str().to_owned(),
            resource: delivery.event().resource().map(|resource| {
                format!(
                    "{}/{}/{}",
                    resource.namespace().as_str(),
                    resource.kind().as_str(),
                    resource.key().as_str()
                )
            }),
        };
        self.deliver(identity.installation_id(), &event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, EventEnvelope, EventName, OperationContext, PublisherId, ResourceKey,
        ResourceKind, ResourceNamespace, ResourceRef,
    };

    #[derive(Default)]
    struct RecordingChannel {
        events: Mutex<Vec<RuntimeEvent>>,
        reject: bool,
    }

    impl RuntimeChannel for RecordingChannel {
        fn deliver(&self, event: &RuntimeEvent) -> Result<(), RuntimeChannelError> {
            if self.reject {
                return Err(RuntimeChannelError::Rejected);
            }
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    fn app_identity() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.cloud").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn registers_delivers_and_unregisters() {
        let registry = RuntimeChannelRegistry::new();
        let installation = InstallationId::new();
        let channel = Arc::new(RecordingChannel::default());

        assert!(registry.register(installation, channel.clone()));
        // A second registration for the same installation is refused.
        assert!(!registry.register(installation, Arc::new(RecordingChannel::default())));
        assert!(registry.is_registered(&installation));

        let event = RuntimeEvent {
            topic: "rumahl.files.changed".into(),
            resource: Some("rumahl.files/file/document-1".into()),
        };
        registry.deliver(&installation, &event).unwrap();
        assert_eq!(channel.events.lock().unwrap().as_slice(), [event.clone()]);
        assert!(registry.unregister(&installation));
        assert_eq!(
            registry.deliver(&installation, &event),
            Err(RuntimeChannelError::Unavailable)
        );
    }

    #[test]
    fn routes_an_event_delivery_to_the_subscriber_channel() {
        let registry = RuntimeChannelRegistry::new();
        let identity = app_identity();
        let channel = Arc::new(RecordingChannel::default());
        registry.register(*identity.installation_id(), channel.clone());

        let resource = ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("document-1").unwrap(),
        );
        let event = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(identity.clone()),
            Some(resource),
        );
        registry
            .deliver_event(&EventDelivery::new(identity.into(), event))
            .unwrap();

        let delivered = channel.events.lock().unwrap();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].topic, "rumahl.files.changed");
        assert_eq!(
            delivered[0].resource.as_deref(),
            Some("rumahl.files/file/document-1")
        );
    }

    #[test]
    fn missing_channel_fails_closed() {
        let registry = RuntimeChannelRegistry::new();
        assert_eq!(
            registry.deliver(
                &InstallationId::new(),
                &RuntimeEvent {
                    topic: "rumahl.apps.installed".into(),
                    resource: None
                }
            ),
            Err(RuntimeChannelError::Unavailable)
        );
    }
}
