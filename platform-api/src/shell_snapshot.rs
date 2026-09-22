use std::error::Error;
use std::fmt;

use rumahl_core::{CorrelationId, Identity, UserId};
use rumahl_ui_contracts::ShellSnapshot;

use crate::PlatformRequest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShellSnapshotQuery;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellSnapshotSubject {
    user_id: UserId,
    correlation_id: CorrelationId,
}

pub trait ShellSnapshotProvider {
    type Error: Error + Send + Sync + 'static;

    fn load_for_user(&self, subject: &ShellSnapshotSubject) -> Result<ShellSnapshot, Self::Error>;
}

#[derive(Debug)]
pub struct AuthenticatedShellSnapshotService<P> {
    provider: P,
}

pub enum ShellSnapshotRequestError<E> {
    DirectUserSessionRequired,
    Provider(E),
}

impl ShellSnapshotSubject {
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }
}

impl<P> AuthenticatedShellSnapshotService<P>
where
    P: ShellSnapshotProvider,
{
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn load(
        &self,
        request: PlatformRequest<ShellSnapshotQuery>,
    ) -> Result<ShellSnapshot, ShellSnapshotRequestError<P::Error>> {
        let (context, _) = request.into_parts();
        let Identity::User(identity) = context.actor() else {
            return Err(ShellSnapshotRequestError::DirectUserSessionRequired);
        };
        if context.session().is_none() || context.user() != Some(identity.id()) {
            return Err(ShellSnapshotRequestError::DirectUserSessionRequired);
        }

        let subject = ShellSnapshotSubject {
            user_id: *identity.id(),
            correlation_id: *context.correlation_id(),
        };
        self.provider
            .load_for_user(&subject)
            .map_err(ShellSnapshotRequestError::Provider)
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }
}

impl<E> fmt::Debug for ShellSnapshotRequestError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectUserSessionRequired => {
                formatter.debug_tuple("DirectUserSessionRequired").finish()
            }
            Self::Provider(_) => formatter.debug_tuple("Provider").finish_non_exhaustive(),
        }
    }
}

impl<E> fmt::Display for ShellSnapshotRequestError<E>
where
    E: Error,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectUserSessionRequired => {
                write!(formatter, "a direct authenticated user session is required")
            }
            Self::Provider(_) => write!(formatter, "shell snapshot is unavailable"),
        }
    }
}

impl<E> Error for ShellSnapshotRequestError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DirectUserSessionRequired => None,
            Self::Provider(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use rumahl_core::{
        AppId, AppIdentity, InstallationId, PublisherId, ServiceId, ServiceIdentity, SessionId,
        UserIdentity,
    };
    use rumahl_ui_contracts::{
        ShellSystemStatus, ShellTheme, ShellUser, SystemProtectionStatus, WindowChromeVariant,
    };

    use crate::{AuthenticatedPrincipal, PlatformRequestGateway, RequestAuthenticator};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "sensitive provider detail")
        }
    }

    impl Error for TestError {}

    struct RecordingProvider {
        subjects: RefCell<Vec<ShellSnapshotSubject>>,
        fail: bool,
    }

    impl ShellSnapshotProvider for RecordingProvider {
        type Error = TestError;

        fn load_for_user(
            &self,
            subject: &ShellSnapshotSubject,
        ) -> Result<ShellSnapshot, Self::Error> {
            self.subjects.borrow_mut().push(*subject);
            if self.fail {
                return Err(TestError);
            }
            snapshot()
        }
    }

    struct FixedAuthenticator(AuthenticatedPrincipal);

    impl RequestAuthenticator for FixedAuthenticator {
        type Credential = str;
        type Error = TestError;

        fn authenticate(
            &self,
            _: &Self::Credential,
        ) -> Result<AuthenticatedPrincipal, Self::Error> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn loads_snapshot_for_direct_authenticated_user() {
        let user_id = UserId::new();
        let service = service();
        let request = gateway_request(AuthenticatedPrincipal::user(
            UserIdentity::new(user_id),
            SessionId::new(),
        ));
        let correlation_id = *request.context().correlation_id();

        let loaded = service.load(request).unwrap();

        assert_eq!(loaded.user().display_name(), "Example User");
        assert_eq!(
            service.provider().subjects.borrow().as_slice(),
            &[ShellSnapshotSubject {
                user_id,
                correlation_id
            }]
        );
    }

    #[test]
    fn rejects_delegated_apps_and_services_before_provider_call() {
        let user_id = UserId::new();
        for principal in [
            AuthenticatedPrincipal::app_as_user(app(), user_id, SessionId::new()),
            AuthenticatedPrincipal::service(ServiceIdentity::new(
                ServiceId::parse("rumahl.shell").unwrap(),
            )),
        ] {
            let service = service();
            let error = service.load(gateway_request(principal)).unwrap_err();
            assert!(matches!(
                error,
                ShellSnapshotRequestError::DirectUserSessionRequired
            ));
            assert!(service.provider().subjects.borrow().is_empty());
        }
    }

    #[test]
    fn preserves_provider_error_as_source_without_exposing_it_in_display_or_debug() {
        let principal =
            AuthenticatedPrincipal::user(UserIdentity::new(UserId::new()), SessionId::new());
        let service = AuthenticatedShellSnapshotService::new(RecordingProvider {
            subjects: RefCell::new(Vec::new()),
            fail: true,
        });

        let error = service.load(gateway_request(principal)).unwrap_err();

        assert_eq!(error.to_string(), "shell snapshot is unavailable");
        assert_eq!(format!("{error:?}"), "Provider(..)");
        assert_eq!(
            error.source().unwrap().to_string(),
            "sensitive provider detail"
        );
    }

    fn service() -> AuthenticatedShellSnapshotService<RecordingProvider> {
        AuthenticatedShellSnapshotService::new(RecordingProvider {
            subjects: RefCell::new(Vec::new()),
            fail: false,
        })
    }

    fn gateway_request(principal: AuthenticatedPrincipal) -> PlatformRequest<ShellSnapshotQuery> {
        PlatformRequestGateway::new(FixedAuthenticator(principal))
            .authenticate("credential", ShellSnapshotQuery)
            .unwrap()
    }

    fn app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn snapshot() -> Result<ShellSnapshot, TestError> {
        Ok(ShellSnapshot::new(
            "shell-build-001",
            "revision-001",
            ShellUser::new("Example User", "en-US").unwrap(),
            ShellTheme::new(
                "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
                WindowChromeVariant::Standard,
            )
            .unwrap(),
            ShellSystemStatus::new(SystemProtectionStatus::Active, 0, 1, None).unwrap(),
            Vec::new(),
        )
        .unwrap())
    }
}
