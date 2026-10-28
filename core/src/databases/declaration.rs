use super::AppDatabaseId;

/// Logical persistent database requested by an app manifest.
///
/// The declaration intentionally contains no engine, host path, endpoint, or
/// credential. The OS chooses the provider and delivery mechanism for the
/// installed runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDatabaseDeclaration {
    id: AppDatabaseId,
}

impl AppDatabaseDeclaration {
    pub fn new(id: AppDatabaseId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &AppDatabaseId {
        &self.id
    }
}
