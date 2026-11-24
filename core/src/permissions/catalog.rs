//! The permission catalog for the OS-owned `rumahl.` namespace.
//!
//! Third-party apps may freely request permissions in their own publisher
//! namespace (`com.example.…`). Permission ids owned by the OS itself are a
//! contract between app, shell and platform: an app can only request the ids
//! this catalog defines, in the scopes the platform actually honors. Anything
//! else is rejected at manifest validation, long before a grant could exist.

use super::{PermissionRequest, PermissionScope};

/// Prefix of the filesystem permission family.
pub const FILES_PERMISSION_PREFIX: &str = "rumahl.files.";
/// Read access to user-selected personal files.
pub const FILES_READ_PERMISSION: &str = "rumahl.files.read";
/// Write access to user-selected personal files.
pub const FILES_WRITE_PERMISSION: &str = "rumahl.files.write";

/// A permission the OS itself defines, with the scopes grants may use.
pub struct OsPermission {
    id: &'static str,
    allowed_scopes: &'static [PermissionScope],
}

impl OsPermission {
    pub const fn id(&self) -> &'static str {
        self.id
    }

    pub const fn allowed_scopes(&self) -> &'static [PermissionScope] {
        self.allowed_scopes
    }

    fn allows_scope(&self, scope: PermissionScope) -> bool {
        self.allowed_scopes.contains(&scope)
    }
}

const READ: OsPermission = OsPermission {
    id: FILES_READ_PERMISSION,
    // Filesystem access is only ever granted for files the user picked.
    // `Explicit` scopes the grant to concrete resource keys; `UserSelected`
    // covers the picker-driven flow where the choice is the resource list.
    allowed_scopes: &[PermissionScope::UserSelected, PermissionScope::Explicit],
};

const WRITE: OsPermission = OsPermission {
    id: FILES_WRITE_PERMISSION,
    allowed_scopes: &[PermissionScope::UserSelected, PermissionScope::Explicit],
};

/// Every OS-owned permission id the platform honors.
pub fn os_permission_catalog() -> &'static [OsPermission] {
    &[READ, WRITE]
}

/// Looks up an OS-owned permission id.
pub fn os_permission(id: &str) -> Option<&'static OsPermission> {
    os_permission_catalog()
        .iter()
        .find(|permission| permission.id() == id)
}

/// True when the permission id belongs to the OS-owned files family and must
/// therefore be validated against the catalog.
pub fn is_files_permission(id: &str) -> bool {
    id.starts_with(FILES_PERMISSION_PREFIX)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsPermissionCatalogError {
    UnknownPermission {
        permission: String,
    },
    ScopeNotAllowed {
        permission: String,
        scope: PermissionScope,
    },
}

/// Validates a manifest's permission request against the OS catalog.
///
/// Unknown OS-owned ids and unsupported scopes are rejected; permissions
/// outside the OS namespaces pass through untouched.
pub fn os_permission_catalog_validate(
    request: &PermissionRequest,
) -> Result<(), OsPermissionCatalogError> {
    let permission = request.permission().as_str();

    if !is_files_permission(permission) {
        return Ok(());
    }

    let Some(definition) = os_permission(permission) else {
        return Err(OsPermissionCatalogError::UnknownPermission {
            permission: permission.to_owned(),
        });
    };

    if !definition.allows_scope(request.requested_scope()) {
        return Err(OsPermissionCatalogError::ScopeNotAllowed {
            permission: permission.to_owned(),
            scope: request.requested_scope(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::PermissionId;

    fn request(permission: &str, scope: PermissionScope) -> PermissionRequest {
        PermissionRequest::new(PermissionId::parse(permission).unwrap(), scope, false, None)
    }

    #[test]
    fn files_permissions_accept_user_selected_and_explicit() {
        for id in [FILES_READ_PERMISSION, FILES_WRITE_PERMISSION] {
            for scope in [PermissionScope::UserSelected, PermissionScope::Explicit] {
                assert!(os_permission_catalog_validate(&request(id, scope)).is_ok());
            }
        }
    }

    #[test]
    fn files_permissions_reject_other_scopes() {
        for scope in [
            PermissionScope::AppPrivate,
            PermissionScope::UserOwn,
            PermissionScope::FamilyShared,
            PermissionScope::System,
        ] {
            assert_eq!(
                os_permission_catalog_validate(&request(FILES_READ_PERMISSION, scope)),
                Err(OsPermissionCatalogError::ScopeNotAllowed {
                    permission: FILES_READ_PERMISSION.to_owned(),
                    scope,
                })
            );
        }
    }

    #[test]
    fn unknown_files_permission_is_rejected() {
        assert_eq!(
            os_permission_catalog_validate(&request(
                "rumahl.files.delete",
                PermissionScope::Explicit
            )),
            Err(OsPermissionCatalogError::UnknownPermission {
                permission: "rumahl.files.delete".to_owned()
            })
        );
    }

    #[test]
    fn third_party_permissions_are_untouched() {
        assert!(
            os_permission_catalog_validate(&request(
                "com.rumahl.notes.export",
                PermissionScope::UserOwn
            ))
            .is_ok()
        );
        assert!(
            os_permission_catalog_validate(&request(
                "rumahl.apps.launch",
                PermissionScope::UserOwn
            ))
            .is_ok()
        );
    }
}
