use crate::{AppId, CapabilityId};

/// Describes a requested action; execution is handled by the dispatcher or runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    InvokeCapability(CapabilityId),
    /// References a logical app; the runtime resolves its installation.
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
