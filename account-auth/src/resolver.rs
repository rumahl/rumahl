use std::error::Error;
use std::fmt;

use rumahl_core::{SessionId, UnixTimestamp};
use rumahl_platform_api::SessionCredentialResolver;

use crate::{
    SessionCredentialRecord, SessionCredentialRepository, SessionToken,
    SessionTokenGenerationError, SessionTokenParseError,
};

#[derive(Debug)]
pub struct SessionCredentialIssuer<R> {
    repository: R,
}

#[derive(Debug)]
pub enum SessionCredentialIssuanceError<RepositoryError> {
    Generation(SessionTokenGenerationError),
    Repository(RepositoryError),
}

#[derive(Debug)]
pub struct StoredSessionCredentialResolver<R> {
    repository: R,
}

#[derive(Debug)]
pub enum StoredSessionCredentialResolverError<RepositoryError> {
    InvalidCredential(SessionTokenParseError),
    Repository(RepositoryError),
    UnknownCredential,
}

impl<R> SessionCredentialIssuer<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn issue(
        &self,
        session_id: SessionId,
        issued_at: UnixTimestamp,
    ) -> Result<SessionToken, SessionCredentialIssuanceError<R::Error>>
    where
        R: SessionCredentialRepository,
    {
        let token = SessionToken::generate().map_err(SessionCredentialIssuanceError::Generation)?;
        let record = SessionCredentialRecord::new(token.digest(), session_id, issued_at);

        self.repository
            .insert(&record)
            .map_err(SessionCredentialIssuanceError::Repository)?;

        Ok(token)
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R> StoredSessionCredentialResolver<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R> SessionCredentialResolver for StoredSessionCredentialResolver<R>
where
    R: SessionCredentialRepository,
{
    type Credential = str;
    type Error = StoredSessionCredentialResolverError<R::Error>;

    fn resolve_session(&self, credential: &Self::Credential) -> Result<SessionId, Self::Error> {
        let token = SessionToken::parse(credential)
            .map_err(StoredSessionCredentialResolverError::InvalidCredential)?;

        self.repository
            .resolve(&token.digest())
            .map_err(StoredSessionCredentialResolverError::Repository)?
            .ok_or(StoredSessionCredentialResolverError::UnknownCredential)
    }
}

impl<RepositoryError> fmt::Display for SessionCredentialIssuanceError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generation(_) => write!(f, "could not generate an OS session credential"),
            Self::Repository(_) => write!(f, "could not persist an OS session credential"),
        }
    }
}

impl<RepositoryError> Error for SessionCredentialIssuanceError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Generation(error) => Some(error),
            Self::Repository(error) => Some(error),
        }
    }
}

impl<RepositoryError> fmt::Display for StoredSessionCredentialResolverError<RepositoryError>
where
    RepositoryError: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredential(_) | Self::UnknownCredential => {
                write!(f, "OS session credential is invalid")
            }
            Self::Repository(_) => write!(f, "OS session credential store is unavailable"),
        }
    }
}

impl<RepositoryError> Error for StoredSessionCredentialResolverError<RepositoryError>
where
    RepositoryError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCredential(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::UnknownCredential => None,
        }
    }
}
