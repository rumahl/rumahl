use std::error::Error;
use std::fmt;

use crate::{CapabilityExecution, EventDelivery, InstalledApp, OperationContext};

use super::{RuntimeKind, RuntimeStatus};

pub trait RuntimeAdapter: Send + Sync {
    fn kind(&self) -> RuntimeKind;

    fn start(
        &self,
        context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError>;

    fn stop(
        &self,
        context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError>;

    fn status(
        &self,
        context: &OperationContext,
        app: &InstalledApp,
    ) -> Result<RuntimeStatus, RuntimeAdapterError>;

    fn execute_capability(
        &self,
        app: &InstalledApp,
        execution: &CapabilityExecution,
    ) -> Result<(), RuntimeAdapterError>;

    fn deliver_event(
        &self,
        app: &InstalledApp,
        delivery: &EventDelivery,
    ) -> Result<(), RuntimeAdapterError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAdapterError {
    Unavailable,
    Rejected,
    Failed,
}

impl fmt::Display for RuntimeAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => {
                write!(f, "runtime adapter is unavailable")
            }

            Self::Rejected => {
                write!(f, "runtime adapter rejected the operation")
            }

            Self::Failed => {
                write!(f, "runtime adapter failed to complete the operation")
            }
        }
    }
}

impl Error for RuntimeAdapterError {}
