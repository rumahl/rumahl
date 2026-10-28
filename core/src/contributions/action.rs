use crate::{AppId, CapabilityId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    InvokeCapability(CapabilityId),
    OpenApp(AppId),
}

impl CommandAction {
    pub fn invoke_capability(capability: CapabilityId) -> Self {
        Self::InvokeCapability(capability)
    }

    pub fn open_app(app_id: AppId) -> Self {
        Self::OpenApp(app_id)
    }

    pub fn capability(&self) -> Option<&CapabilityId> {
        match self {
            Self::InvokeCapability(capability) => Some(capability),

            Self::OpenApp(_) => None,
        }
    }

    pub fn app_id(&self) -> Option<&AppId> {
        match self {
            Self::OpenApp(app_id) => Some(app_id),

            Self::InvokeCapability(_) => None,
        }
    }
}
