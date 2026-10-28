use crate::AppIdentity;

use super::{AppDatabaseDeclaration, AppDatabaseId};

/// Active logical database binding owned by one concrete app installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDatabaseBinding {
    owner: AppIdentity,
    declaration: AppDatabaseDeclaration,
}

impl AppDatabaseBinding {
    pub fn new(owner: AppIdentity, declaration: AppDatabaseDeclaration) -> Self {
        Self { owner, declaration }
    }

    pub fn owner(&self) -> &AppIdentity {
        &self.owner
    }

    pub fn declaration(&self) -> &AppDatabaseDeclaration {
        &self.declaration
    }

    pub fn id(&self) -> &AppDatabaseId {
        self.declaration.id()
    }
}
