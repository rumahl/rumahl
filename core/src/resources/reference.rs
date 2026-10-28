use std::fmt;

use super::{ResourceKey, ResourceKind, ResourceNamespace};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    namespace: ResourceNamespace,
    kind: ResourceKind,
    key: ResourceKey,
}

impl ResourceRef {
    pub fn new(namespace: ResourceNamespace, kind: ResourceKind, key: ResourceKey) -> Self {
        Self {
            namespace,
            kind,
            key,
        }
    }

    pub fn namespace(&self) -> &ResourceNamespace {
        &self.namespace
    }

    pub fn kind(&self) -> &ResourceKind {
        &self.kind
    }

    pub fn key(&self) -> &ResourceKey {
        &self.key
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.namespace, self.kind, self.key,)
    }
}
