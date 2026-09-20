use std::error::Error;
use std::fmt;

use super::{RuntimeAdapter, RuntimeKind};

#[derive(Default)]
pub struct RuntimeAdapterRegistry {
    adapters: Vec<Box<dyn RuntimeAdapter>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAdapterRegistryError {
    AdapterAlreadyRegistered(RuntimeKind),
}

impl RuntimeAdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(
        &self,
        adapter: &dyn RuntimeAdapter,
    ) -> Result<(), RuntimeAdapterRegistryError> {
        if self.adapter(adapter.kind()).is_some() {
            return Err(RuntimeAdapterRegistryError::AdapterAlreadyRegistered(
                adapter.kind(),
            ));
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        adapter: Box<dyn RuntimeAdapter>,
    ) -> Result<(), RuntimeAdapterRegistryError> {
        self.can_register(adapter.as_ref())?;

        self.adapters.push(adapter);

        Ok(())
    }

    pub fn adapter(&self, kind: RuntimeKind) -> Option<&dyn RuntimeAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.kind() == kind)
            .map(Box::as_ref)
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

impl fmt::Display for RuntimeAdapterRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterAlreadyRegistered(kind) => {
                write!(f, "runtime adapter for '{kind:?}' is already registered")
            }
        }
    }
}

impl Error for RuntimeAdapterRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{CapabilityExecution, EventDelivery, InstalledApp, RuntimeAdapterError};

    struct TestAdapter {
        kind: RuntimeKind,
    }

    impl TestAdapter {
        fn new(kind: RuntimeKind) -> Self {
            Self { kind }
        }
    }

    impl RuntimeAdapter for TestAdapter {
        fn kind(&self) -> RuntimeKind {
            self.kind
        }

        fn execute_capability(
            &self,
            _app: &InstalledApp,
            _execution: &CapabilityExecution,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }

        fn deliver_event(
            &self,
            _app: &InstalledApp,
            _delivery: &EventDelivery,
        ) -> Result<(), RuntimeAdapterError> {
            Ok(())
        }
    }

    #[test]
    fn registry_starts_empty() {
        let registry = RuntimeAdapterRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registers_adapter_by_runtime_kind() {
        let mut registry = RuntimeAdapterRegistry::new();

        registry
            .register(Box::new(TestAdapter::new(RuntimeKind::Web)))
            .unwrap();

        assert!(registry.adapter(RuntimeKind::Web).is_some());
        assert!(registry.adapter(RuntimeKind::Container).is_none());
    }

    #[test]
    fn can_register_without_mutating_registry() {
        let registry = RuntimeAdapterRegistry::new();

        let adapter = TestAdapter::new(RuntimeKind::Container);

        assert!(registry.can_register(&adapter).is_ok());
        assert!(registry.is_empty());
    }

    #[test]
    fn rejects_second_adapter_for_same_runtime_kind() {
        let mut registry = RuntimeAdapterRegistry::new();

        registry
            .register(Box::new(TestAdapter::new(RuntimeKind::Native)))
            .unwrap();

        let result = registry.register(Box::new(TestAdapter::new(RuntimeKind::Native)));

        assert_eq!(
            result.unwrap_err(),
            RuntimeAdapterRegistryError::AdapterAlreadyRegistered(RuntimeKind::Native)
        );
    }
}
