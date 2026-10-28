mod key;
mod kind;
mod namespace;
mod reference;

pub use key::{ResourceKey, ResourceKeyError};

pub use kind::{ResourceKind, ResourceKindError};

pub use namespace::{ResourceNamespace, ResourceNamespaceError};

pub use reference::ResourceRef;
