use super::{
    PermissionId,
    PermissionScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    permission: PermissionId,
    requested_scope: PermissionScope,
    required: bool,
    reason: Option<String>,
}

impl PermissionRequest {
    pub fn new(
        permission: PermissionId,
        requested_scope: PermissionScope,
        required: bool,
        reason: Option<String>,
    ) -> Self {
        Self {
            permission,
            requested_scope,
            required,
            reason,
        }
    }

    pub fn permission(&self) -> &PermissionId {
        &self.permission
    }

    pub fn requested_scope(&self) -> PermissionScope {
        self.requested_scope
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}