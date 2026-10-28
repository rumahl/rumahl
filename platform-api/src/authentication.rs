use std::error::Error;

use rumahl_core::{
    AppIdentity, Identity, OperationContext, ServiceIdentity, SessionId, UserId, UserIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticatedPrincipal {
    User {
        user: UserIdentity,
        session: SessionId,
    },
    AppAsUser {
        app: AppIdentity,
        user: UserId,
        session: SessionId,
    },
    BackgroundApp {
        app: AppIdentity,
    },
    Service {
        service: ServiceIdentity,
    },
}

impl AuthenticatedPrincipal {
    pub fn user(user: UserIdentity, session: SessionId) -> Self {
        Self::User { user, session }
    }

    pub fn app_as_user(app: AppIdentity, user: UserId, session: SessionId) -> Self {
        Self::AppAsUser { app, user, session }
    }

    pub fn background_app(app: AppIdentity) -> Self {
        Self::BackgroundApp { app }
    }

    pub fn service(service: ServiceIdentity) -> Self {
        Self::Service { service }
    }

    pub fn actor(&self) -> Identity {
        match self {
            Self::User { user, .. } => (*user).into(),
            Self::AppAsUser { app, .. } | Self::BackgroundApp { app } => app.clone().into(),
            Self::Service { service } => service.clone().into(),
        }
    }
}

pub trait RequestAuthenticator {
    type Credential: ?Sized;
    type Error: Error + Send + Sync + 'static;

    fn authenticate(
        &self,
        credential: &Self::Credential,
    ) -> Result<AuthenticatedPrincipal, Self::Error>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RequestContextFactory;

impl RequestContextFactory {
    pub fn new() -> Self {
        Self
    }

    pub fn create(&self, principal: AuthenticatedPrincipal) -> OperationContext {
        match principal {
            AuthenticatedPrincipal::User { user, session } => {
                OperationContext::for_user(user, session)
            }
            AuthenticatedPrincipal::AppAsUser { app, user, session } => {
                OperationContext::for_app_as_user(app, user, session)
            }
            AuthenticatedPrincipal::BackgroundApp { app } => {
                OperationContext::for_background_app(app)
            }
            AuthenticatedPrincipal::Service { service } => OperationContext::for_service(service),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{AppId, InstallationId, PublisherId, ServiceId};

    fn app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn creates_direct_user_context() {
        let user = UserIdentity::new(UserId::new());
        let user_id = *user.id();
        let session = SessionId::new();

        let context =
            RequestContextFactory::new().create(AuthenticatedPrincipal::user(user, session));

        assert!(context.actor().is_user());
        assert_eq!(context.user(), Some(&user_id));
        assert_eq!(context.session(), Some(&session));
    }

    #[test]
    fn creates_app_context_only_with_explicit_user_delegation() {
        let user = UserId::new();
        let session = SessionId::new();

        let delegated = RequestContextFactory::new().create(AuthenticatedPrincipal::app_as_user(
            app(),
            user,
            session,
        ));
        let background =
            RequestContextFactory::new().create(AuthenticatedPrincipal::background_app(app()));

        assert!(delegated.actor().is_app());
        assert_eq!(delegated.user(), Some(&user));
        assert_eq!(delegated.session(), Some(&session));
        assert!(background.actor().is_app());
        assert_eq!(background.user(), None);
        assert_eq!(background.session(), None);
    }

    #[test]
    fn creates_service_context_without_user_session() {
        let service = ServiceIdentity::new(ServiceId::parse("rumahl.storage").unwrap());

        let context = RequestContextFactory::new().create(AuthenticatedPrincipal::service(service));

        assert!(context.actor().is_service());
        assert_eq!(context.user(), None);
        assert_eq!(context.session(), None);
    }
}
