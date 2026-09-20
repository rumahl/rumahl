use std::error::Error;
use std::fmt;

use crate::{PlatformRequest, RequestAuthenticator, RequestContextFactory};

#[derive(Debug)]
pub struct PlatformRequestGateway<A> {
    authenticator: A,
    context_factory: RequestContextFactory,
}

pub struct PlatformRequestAuthenticationError<E> {
    source: E,
}

impl<E> fmt::Debug for PlatformRequestAuthenticationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlatformRequestAuthenticationError")
            .finish_non_exhaustive()
    }
}

impl<A> PlatformRequestGateway<A>
where
    A: RequestAuthenticator,
{
    pub fn new(authenticator: A) -> Self {
        Self {
            authenticator,
            context_factory: RequestContextFactory::new(),
        }
    }

    pub fn authenticate<T>(
        &self,
        credential: &A::Credential,
        payload: T,
    ) -> Result<PlatformRequest<T>, PlatformRequestAuthenticationError<A::Error>> {
        let principal = self
            .authenticator
            .authenticate(credential)
            .map_err(PlatformRequestAuthenticationError::new)?;
        let context = self.context_factory.create(principal);

        Ok(PlatformRequest::new(context, payload))
    }

    pub fn authenticator(&self) -> &A {
        &self.authenticator
    }
}

impl<E> PlatformRequestAuthenticationError<E> {
    fn new(source: E) -> Self {
        Self { source }
    }

    pub fn authentication_error(&self) -> &E {
        &self.source
    }
}

impl<E> fmt::Display for PlatformRequestAuthenticationError<E>
where
    E: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "platform request authentication failed")
    }
}

impl<E> Error for PlatformRequestAuthenticationError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthenticatedPrincipal;
    use rumahl_core::{SessionId, UserId, UserIdentity};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct InvalidCredential;

    impl fmt::Display for InvalidCredential {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "invalid credential")
        }
    }

    impl Error for InvalidCredential {}

    struct TestAuthenticator {
        user: UserIdentity,
        session: SessionId,
    }

    impl RequestAuthenticator for TestAuthenticator {
        type Credential = str;
        type Error = InvalidCredential;

        fn authenticate(
            &self,
            credential: &Self::Credential,
        ) -> Result<AuthenticatedPrincipal, Self::Error> {
            if credential != "valid" {
                return Err(InvalidCredential);
            }

            Ok(AuthenticatedPrincipal::user(self.user, self.session))
        }
    }

    fn gateway() -> PlatformRequestGateway<TestAuthenticator> {
        PlatformRequestGateway::new(TestAuthenticator {
            user: UserIdentity::new(UserId::new()),
            session: SessionId::new(),
        })
    }

    #[test]
    fn authenticates_transport_payload_and_creates_context() {
        let request = gateway()
            .authenticate("valid", "list-installed-apps")
            .unwrap();

        assert_eq!(request.payload(), &"list-installed-apps");
        assert!(request.context().actor().is_user());
        assert!(request.context().user().is_some());
        assert!(request.context().session().is_some());
    }

    #[test]
    fn rejects_request_before_context_creation() {
        let error = gateway()
            .authenticate("invalid", "list-installed-apps")
            .unwrap_err();

        assert_eq!(error.authentication_error(), &InvalidCredential);
        assert_eq!(error.to_string(), "platform request authentication failed");
        assert_eq!(
            format!("{error:?}"),
            "PlatformRequestAuthenticationError { .. }"
        );
    }

    #[test]
    fn each_authenticated_request_gets_new_correlation_id() {
        let gateway = gateway();
        let first = gateway.authenticate("valid", ()).unwrap();
        let second = gateway.authenticate("valid", ()).unwrap();

        assert_ne!(
            first.context().correlation_id(),
            second.context().correlation_id()
        );
    }
}
