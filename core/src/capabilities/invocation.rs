use crate::{Identity, OperationContext};

use super::{CapabilityId, CapabilityProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityInvocation {
    context: OperationContext,
    provider: CapabilityProvider,
}

impl CapabilityInvocation {
    pub fn new(context: OperationContext, provider: CapabilityProvider) -> Self {
        Self { context, provider }
    }

    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn capability(&self) -> &CapabilityId {
        self.provider.capability()
    }

    pub fn provider(&self) -> &Identity {
        self.provider.identity()
    }

    pub fn capability_provider(&self) -> &CapabilityProvider {
        &self.provider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, InstallationId, PublisherId, ServiceId, ServiceIdentity};

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn creates_invocation_for_provider() {
        let caller = notes_app();

        let provider_service =
            ServiceIdentity::new(ServiceId::parse("rumahl.search-service").unwrap());

        let provider_identity = provider_service.clone().into();

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        let provider =
            CapabilityProvider::new(provider_service.into(), capability.clone()).unwrap();

        let context = OperationContext::for_background_app(caller);

        let invocation = CapabilityInvocation::new(context, provider);

        assert_eq!(invocation.capability(), &capability);

        assert_eq!(invocation.provider(), &provider_identity);
    }
}
