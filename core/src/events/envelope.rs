use crate::{OperationContext, ResourceRef};

use super::{EventId, EventName};

#[derive(Debug, Clone)]
pub struct EventEnvelope {
    id: EventId,
    name: EventName,
    context: OperationContext,
    resource: Option<ResourceRef>,
}

impl EventEnvelope {
    pub fn new(name: EventName, context: OperationContext, resource: Option<ResourceRef>) -> Self {
        Self {
            id: EventId::new(),
            name,
            context,
            resource,
        }
    }

    pub fn id(&self) -> EventId {
        self.id
    }

    pub fn name(&self) -> &EventName {
        &self.name
    }

    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn resource(&self) -> Option<&ResourceRef> {
        self.resource.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, InstallationId, PublisherId, ResourceKey, ResourceKind,
        ResourceNamespace,
    };

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn file(key: &str) -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse(key).unwrap(),
        )
    }

    #[test]
    fn creates_event_envelope() {
        let name = EventName::parse("rumahl.files.changed").unwrap();

        let resource = file("document-1");

        let event = EventEnvelope::new(
            name.clone(),
            OperationContext::for_background_app(notes_app()),
            Some(resource.clone()),
        );

        assert_eq!(event.name(), &name);

        assert_eq!(event.resource(), Some(&resource));
    }

    #[test]
    fn event_has_unique_id() {
        let first = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(notes_app()),
            None,
        );

        let second = EventEnvelope::new(
            EventName::parse("rumahl.files.changed").unwrap(),
            OperationContext::for_background_app(notes_app()),
            None,
        );

        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn event_can_exist_without_resource() {
        let event = EventEnvelope::new(
            EventName::parse("rumahl.apps.installed").unwrap(),
            OperationContext::for_background_app(notes_app()),
            None,
        );

        assert_eq!(event.resource(), None);
    }
}
