use crate::identity::{
    AppIdentity,
    Identity,
    ServiceIdentity,
    SessionId,
    UserId,
    UserIdentity,
};

use super::CorrelationId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationContext {
    actor: Identity,
    user: Option<UserId>,
    session: Option<SessionId>,
    correlation_id: CorrelationId,
}

impl OperationContext {
    pub fn for_user(
        user: UserIdentity,
        session: SessionId,
    ) -> Self {
        let user_id = *user.id();

        Self {
            actor: user.into(),
            user: Some(user_id),
            session: Some(session),
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn for_app_as_user(
        app: AppIdentity,
        user: UserId,
        session: SessionId,
    ) -> Self {
        Self {
            actor: app.into(),
            user: Some(user),
            session: Some(session),
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn for_background_app(
        app: AppIdentity,
    ) -> Self {
        Self {
            actor: app.into(),
            user: None,
            session: None,
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn for_service(
        service: ServiceIdentity,
    ) -> Self {
        Self {
            actor: service.into(),
            user: None,
            session: None,
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn continue_as(
        &self,
        actor: impl Into<Identity>,
    ) -> Self {
        Self {
            actor: actor.into(),
            user: self.user,
            session: self.session,
            correlation_id: self.correlation_id,
        }
    }

    pub fn actor(&self) -> &Identity {
        &self.actor
    }

    pub fn user(&self) -> Option<&UserId> {
        self.user.as_ref()
    }

    pub fn session(&self) -> Option<&SessionId> {
        self.session.as_ref()
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{
        AppId,
        InstallationId,
        PublisherId,
        ServiceId,
    };

    #[test]
    fn creates_user_operation_context() {
        let user =
            UserIdentity::new(UserId::new());

        let user_id = *user.id();

        let session =
            SessionId::new();

        let context =
            OperationContext::for_user(
                user,
                session,
            );

        assert!(context.actor().is_user());

        assert_eq!(
            context.user(),
            Some(&user_id)
        );

        assert_eq!(
            context.session(),
            Some(&session)
        );
    }

        #[test]
    fn app_can_run_in_user_context() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let user = UserId::new();
        let session = SessionId::new();

        let context =
            OperationContext::for_app_as_user(
                app,
                user,
                session,
            );

        assert!(context.actor().is_app());

        assert_eq!(
            context.user(),
            Some(&user)
        );

        assert_eq!(
            context.session(),
            Some(&session)
        );
    }

        #[test]
    fn continuing_operation_preserves_correlation() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let service =
            ServiceIdentity::new(
                ServiceId::parse("rumahl.storage").unwrap(),
            );

        let context =
            OperationContext::for_app_as_user(
                app,
                UserId::new(),
                SessionId::new(),
            );

        let original_correlation =
            *context.correlation_id();

        let service_context =
            context.continue_as(service);

        assert!(service_context.actor().is_service());

        assert_eq!(
            service_context.correlation_id(),
            &original_correlation
        );

        assert_eq!(
            service_context.user(),
            context.user()
        );

        assert_eq!(
            service_context.session(),
            context.session()
        );
    }
}