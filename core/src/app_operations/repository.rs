use std::error::Error;

use super::{AppOperation, AppOperationId};

/// Durable journal boundary for cross-resource app operations.
///
/// `store_transition` must use the operation revision as an optimistic lock:
/// revision zero is inserted with `create`, and each stored transition replaces
/// exactly the preceding revision or fails without changing the journal.
pub trait AppOperationRepository {
    type Error: Error + Send + Sync + 'static;

    fn create(&self, operation: &AppOperation) -> Result<(), Self::Error>;

    fn store_transition(&self, operation: &AppOperation) -> Result<(), Self::Error>;

    fn find(&self, id: &AppOperationId) -> Result<Option<AppOperation>, Self::Error>;

    fn list_incomplete(&self) -> Result<Vec<AppOperation>, Self::Error>;
}
