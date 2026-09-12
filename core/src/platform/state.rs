use crate::{CapabilityRegistry, CommandRegistry, ContributionRegistry, EventBus, SearchRegistry};

#[derive(Debug, Default, Clone)]
pub struct PlatformState {
    capability_registry: CapabilityRegistry,
    contribution_registry: ContributionRegistry,
    command_registry: CommandRegistry,
    search_registry: SearchRegistry,
    event_bus: EventBus,
}

impl PlatformState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn capability_registry(&self) -> &CapabilityRegistry {
        &self.capability_registry
    }

    pub fn contribution_registry(&self) -> &ContributionRegistry {
        &self.contribution_registry
    }

    pub fn command_registry(&self) -> &CommandRegistry {
        &self.command_registry
    }

    pub fn search_registry(&self) -> &SearchRegistry {
        &self.search_registry
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub(crate) fn capability_registry_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.capability_registry
    }

    pub(crate) fn contribution_registry_mut(&mut self) -> &mut ContributionRegistry {
        &mut self.contribution_registry
    }

    pub(crate) fn command_registry_mut(&mut self) -> &mut CommandRegistry {
        &mut self.command_registry
    }

    pub(crate) fn search_registry_mut(&mut self) -> &mut SearchRegistry {
        &mut self.search_registry
    }

    pub(crate) fn event_bus_mut(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }
}
