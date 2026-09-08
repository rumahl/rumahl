use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use crate::identity::{
    Identity,
    ServiceId,
    UserId,
};

use super::{
    PermissionScope,
    UserRole,
};

#[derive(Debug, Default)]
pub struct GrantIssuerPolicy {
    user_access: HashMap<UserId, UserRole>,
    trusted_services: HashSet<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantIssuerPolicyError {
    AppCannotIssueGrant,
    UserAccessNotAssigned,
    UserAccessInsufficient,
    ServiceNotTrusted,
}

impl GrantIssuerPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_user_access(
        &mut self,
        user: UserId,
        access: UserRole,
    ) {
        self.user_access.insert(user, access);
    }

    pub fn trust_service(
        &mut self,
        service: ServiceId,
    ) {
        self.trusted_services.insert(service);
    }

    pub fn authorize_issuer(
        &self,
        issuer: &Identity,
        scope: PermissionScope,
    ) -> Result<(), GrantIssuerPolicyError> {
        match issuer {
            Identity::App(_) => {
                Err(
                    GrantIssuerPolicyError::AppCannotIssueGrant
                )
            }

            Identity::Service(service) => {
                if self.trusted_services.contains(service.id()) {
                    Ok(())
                } else {
                    Err(
                        GrantIssuerPolicyError::ServiceNotTrusted
                    )
                }
            }

            Identity::User(user) => {
                let access = self
                    .user_access
                    .get(user.id())
                    .copied()
                    .ok_or(
                        GrantIssuerPolicyError::UserAccessNotAssigned
                    )?;

                Self::authorize_user(access, scope)
            }
        }
    }

    fn authorize_user(
        access: UserRole,
        scope: PermissionScope,
    ) -> Result<(), GrantIssuerPolicyError> {
        let allowed = match access {
            UserRole::PreAuthentication => false,

            UserRole::User => {
                matches!(
                    scope,
                    PermissionScope::UserOwn
                        | PermissionScope::UserSelected
                        | PermissionScope::Explicit
                )
            }

            UserRole::Manager => {
                matches!(
                    scope,
                    PermissionScope::UserOwn
                        | PermissionScope::UserSelected
                        | PermissionScope::Explicit
                        | PermissionScope::FamilyShared
                )
            }

            UserRole::Administrator => {
                matches!(
                    scope,
                    PermissionScope::UserOwn
                        | PermissionScope::UserSelected
                        | PermissionScope::Explicit
                        | PermissionScope::FamilyShared
                )
            }

            UserRole::Owner => true,
        };

        if allowed {
            Ok(())
        } else {
            Err(
                GrantIssuerPolicyError::UserAccessInsufficient
            )
        }
    }
}

impl fmt::Display for GrantIssuerPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppCannotIssueGrant => {
                write!(
                    f,
                    "apps are not allowed to issue permission grants"
                )
            }

            Self::UserAccessNotAssigned => {
                write!(
                    f,
                    "user has no assigned access class"
                )
            }

            Self::UserAccessInsufficient => {
                write!(
                    f,
                    "user access class is insufficient for this permission scope"
                )
            }

            Self::ServiceNotTrusted => {
                write!(
                    f,
                    "service is not trusted to issue permission grants"
                )
            }
        }
    }
}

impl Error for GrantIssuerPolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        ServiceIdentity,
        UserIdentity,
    };

    #[test]
    fn normal_user_can_issue_explicit_grant() {
        let user = UserIdentity::new(UserId::new());
        let user_id = *user.id();

        let mut policy =
            GrantIssuerPolicy::new();

        policy.set_user_access(
            user_id,
            UserRole::User,
        );

        assert_eq!(
            policy.authorize_issuer(
                &user.into(),
                PermissionScope::Explicit
            ),
            Ok(())
        );
    }

    #[test]
    fn normal_user_cannot_issue_system_grant() {
        let user = UserIdentity::new(UserId::new());
        let user_id = *user.id();

        let mut policy =
            GrantIssuerPolicy::new();

        policy.set_user_access(
            user_id,
            UserRole::User,
        );

        assert_eq!(
            policy.authorize_issuer(
                &user.into(),
                PermissionScope::System
            ),
            Err(
                GrantIssuerPolicyError::UserAccessInsufficient
            )
        );
    }

    #[test]
    fn owner_can_issue_system_grant() {
        let user = UserIdentity::new(UserId::new());
        let user_id = *user.id();

        let mut policy =
            GrantIssuerPolicy::new();

        policy.set_user_access(
            user_id,
            UserRole::Owner,
        );

        assert_eq!(
            policy.authorize_issuer(
                &user.into(),
                PermissionScope::System
            ),
            Ok(())
        );
    }

    #[test]
    fn untrusted_service_cannot_issue_grant() {
        let service =
            ServiceIdentity::new(
                ServiceId::parse(
                    "rumahl.random-service"
                )
                .unwrap(),
            );

        let policy =
            GrantIssuerPolicy::new();

        assert_eq!(
            policy.authorize_issuer(
                &service.into(),
                PermissionScope::System
            ),
            Err(
                GrantIssuerPolicyError::ServiceNotTrusted
            )
        );
    }

    #[test]
    fn trusted_service_can_issue_grant() {
        let service_id =
            ServiceId::parse(
                "rumahl.permission-service"
            )
            .unwrap();

        let service =
            ServiceIdentity::new(
                service_id.clone()
            );

        let mut policy =
            GrantIssuerPolicy::new();

        policy.trust_service(service_id);

        assert_eq!(
            policy.authorize_issuer(
                &service.into(),
                PermissionScope::System
            ),
            Ok(())
        );
    }
}