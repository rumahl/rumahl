use super::{AppIdentity, ServiceIdentity, UserIdentity};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identity {
    User(UserIdentity),
    App(AppIdentity),
    Service(ServiceIdentity),
}

impl Identity {
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }

    pub fn is_app(&self) -> bool {
        matches!(self, Self::App(_))
    }

    pub fn is_service(&self) -> bool {
        matches!(self, Self::Service(_))
    }
}

impl From<UserIdentity> for Identity {
    fn from(identity: UserIdentity) -> Self {
        Self::User(identity)
    }
}

impl From<AppIdentity> for Identity {
    fn from(identity: AppIdentity) -> Self {
        Self::App(identity)
    }
}

impl From<ServiceIdentity> for Identity {
    fn from(identity: ServiceIdentity) -> Self {
        Self::Service(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::identity::{AppId, InstallationId, PublisherId, ServiceId, UserId};

    #[test]
    fn converts_user_identity_into_identity() {
        let user = UserIdentity::new(UserId::new());

        let identity: Identity = user.into();

        assert!(identity.is_user());
        assert!(!identity.is_app());
        assert!(!identity.is_service());
    }

    #[test]
    fn converts_app_identity_into_identity() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let identity: Identity = app.into();

        assert!(identity.is_app());
        assert!(!identity.is_user());
        assert!(!identity.is_service());
    }

    #[test]
    fn converts_service_identity_into_identity() {
        let service = ServiceIdentity::new(ServiceId::parse("rumahl.storage").unwrap());

        let identity: Identity = service.into();

        assert!(identity.is_service());
        assert!(!identity.is_user());
        assert!(!identity.is_app());
    }
}
