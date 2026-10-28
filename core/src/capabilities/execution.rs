use crate::{OperationContext, ResourceRef};

use super::{CapabilityId, CapabilityProvider};

#[derive(Debug, Clone)]
pub struct CapabilityExecution {
    context: OperationContext,
    provider: CapabilityProvider,
    resource: Option<ResourceRef>,
}

impl CapabilityExecution {
    pub(crate) fn new(
        context: OperationContext,
        provider: CapabilityProvider,
        resource: Option<ResourceRef>,
    ) -> Self {
        Self {
            context,
            provider,
            resource,
        }
    }

    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn capability(&self) -> &CapabilityId {
        self.provider.capability()
    }

    pub fn provider(&self) -> &CapabilityProvider {
        &self.provider
    }

    pub fn resource(&self) -> Option<&ResourceRef> {
        self.resource.as_ref()
    }
}
