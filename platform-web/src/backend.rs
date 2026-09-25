use std::sync::Arc;

use rumahl_core::{Identity, SessionId, UserId};
use rumahl_platform_api::{
    AuthenticatedShellSnapshotService, LocalSessionAuthenticationError, PlatformRequestGateway,
    RequestAuthenticator, ShellSnapshotProvider, ShellSnapshotQuery, ShellSnapshotRequestError,
};
use rumahl_ui_contracts::ShellSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellIdentity {
    pub user_id: UserId,
    pub session_id: SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellBackendError {
    Unauthorized,
    Unavailable,
}

/// Classify authentication failures without exposing sensitive details to HTTP.
/// A custom authenticator supplies its own implementation.
pub trait ShellAuthenticationError {
    fn category(&self) -> ShellBackendError;
}

impl<R, S, C> ShellAuthenticationError for LocalSessionAuthenticationError<R, S, C> {
    fn category(&self) -> ShellBackendError {
        match self {
            Self::Credential(_) | Self::InvalidSession => ShellBackendError::Unauthorized,
            Self::Repository(_) | Self::Clock(_) => ShellBackendError::Unavailable,
        }
    }
}

/// The transport receives only an opaque browser credential. Implementations
/// must revalidate it on every call; WebSocket connections call this repeatedly.
pub trait ShellBackend: Send + Sync + 'static {
    fn authenticate(&self, credential: &str) -> Result<ShellIdentity, ShellBackendError>;
    fn snapshot(&self, credential: &str) -> Result<ShellSnapshot, ShellBackendError>;
}

pub struct PlatformShellBackend<A, P> {
    gateway: PlatformRequestGateway<A>,
    snapshots: AuthenticatedShellSnapshotService<P>,
}

impl<A, P> PlatformShellBackend<A, P>
where
    A: RequestAuthenticator,
    P: ShellSnapshotProvider,
{
    pub fn new(authenticator: A, provider: P) -> Self {
        Self {
            gateway: PlatformRequestGateway::new(authenticator),
            snapshots: AuthenticatedShellSnapshotService::new(provider),
        }
    }
}

impl<A, P> ShellBackend for PlatformShellBackend<A, P>
where
    A: RequestAuthenticator<Credential = str> + Send + Sync + 'static,
    A::Error: ShellAuthenticationError,
    P: ShellSnapshotProvider + Send + Sync + 'static,
{
    fn authenticate(&self, credential: &str) -> Result<ShellIdentity, ShellBackendError> {
        let request = self
            .gateway
            .authenticate(credential, ())
            .map_err(|error| error.authentication_error().category())?;
        let context = request.context();
        let Identity::User(user) = context.actor() else {
            return Err(ShellBackendError::Unauthorized);
        };
        let session_id = *context.session().ok_or(ShellBackendError::Unauthorized)?;
        if context.user() != Some(user.id()) {
            return Err(ShellBackendError::Unauthorized);
        }
        Ok(ShellIdentity {
            user_id: *user.id(),
            session_id,
        })
    }

    fn snapshot(&self, credential: &str) -> Result<ShellSnapshot, ShellBackendError> {
        let request = self
            .gateway
            .authenticate(credential, ShellSnapshotQuery)
            .map_err(|error| error.authentication_error().category())?;
        self.snapshots.load(request).map_err(|error| match error {
            ShellSnapshotRequestError::DirectUserSessionRequired => ShellBackendError::Unauthorized,
            ShellSnapshotRequestError::Provider(_) => ShellBackendError::Unavailable,
        })
    }
}

impl<T: ShellBackend + ?Sized> ShellBackend for Arc<T> {
    fn authenticate(&self, credential: &str) -> Result<ShellIdentity, ShellBackendError> {
        (**self).authenticate(credential)
    }

    fn snapshot(&self, credential: &str) -> Result<ShellSnapshot, ShellBackendError> {
        (**self).snapshot(credential)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_session_storage_failure_is_not_reported_as_logout() {
        type Failure =
            LocalSessionAuthenticationError<std::io::Error, std::io::Error, std::io::Error>;
        assert_eq!(
            Failure::InvalidSession.category(),
            ShellBackendError::Unauthorized
        );
        assert_eq!(
            Failure::Repository(std::io::Error::other("database offline")).category(),
            ShellBackendError::Unavailable,
        );
        assert_eq!(
            Failure::Clock(std::io::Error::other("clock unavailable")).category(),
            ShellBackendError::Unavailable,
        );
    }
}
