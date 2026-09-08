use std::error::Error;
use std::fmt;

use super::{CapabilityAccessRule, CapabilityId};

#[derive(Debug, Default)]
pub struct CapabilityAccessRegistry {
    rules: Vec<CapabilityAccessRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAccessRegistryError {
    CapabilityAlreadyRegistered,
}

impl CapabilityAccessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        rule: CapabilityAccessRule,
    ) -> Result<(), CapabilityAccessRegistryError> {
        if self
            .rules
            .iter()
            .any(|existing| existing.capability() == rule.capability())
        {
            return Err(CapabilityAccessRegistryError::CapabilityAlreadyRegistered);
        }

        self.rules.push(rule);

        Ok(())
    }

    pub fn rule_for(&self, capability: &CapabilityId) -> Option<&CapabilityAccessRule> {
        self.rules
            .iter()
            .find(|rule| rule.capability() == capability)
    }

    pub fn rules(&self) -> &[CapabilityAccessRule] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl fmt::Display for CapabilityAccessRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityAlreadyRegistered => {
                write!(f, "capability already has an access rule")
            }
        }
    }
}

impl Error for CapabilityAccessRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::PermissionId;

    fn preview_rule() -> CapabilityAccessRule {
        CapabilityAccessRule::new(
            CapabilityId::parse("rumahl.files.preview").unwrap(),
            PermissionId::parse("rumahl.files.read").unwrap(),
        )
    }

    #[test]
    fn registers_access_rule() {
        let mut registry = CapabilityAccessRegistry::new();

        assert!(registry.is_empty());

        registry.register(preview_rule()).unwrap();

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn resolves_access_rule_for_capability() {
        let capability = CapabilityId::parse("rumahl.files.preview").unwrap();

        let mut registry = CapabilityAccessRegistry::new();

        registry.register(preview_rule()).unwrap();

        let rule = registry.rule_for(&capability).unwrap();

        assert_eq!(rule.capability(), &capability);

        assert_eq!(
            rule.permission(),
            &PermissionId::parse("rumahl.files.read").unwrap()
        );
    }

    #[test]
    fn returns_none_for_unknown_capability() {
        let mut registry = CapabilityAccessRegistry::new();

        registry.register(preview_rule()).unwrap();

        let search = CapabilityId::parse("rumahl.search.query").unwrap();

        assert!(registry.rule_for(&search).is_none());
    }

    #[test]
    fn rejects_second_rule_for_same_capability() {
        let capability = CapabilityId::parse("rumahl.files.preview").unwrap();

        let first = CapabilityAccessRule::new(
            capability.clone(),
            PermissionId::parse("rumahl.files.read").unwrap(),
        );

        let second = CapabilityAccessRule::new(
            capability,
            PermissionId::parse("rumahl.files.write").unwrap(),
        );

        let mut registry = CapabilityAccessRegistry::new();

        registry.register(first).unwrap();

        assert_eq!(
            registry.register(second).unwrap_err(),
            CapabilityAccessRegistryError::CapabilityAlreadyRegistered
        );
    }
}
