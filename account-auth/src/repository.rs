use std::error::Error;

use rumahl_core::{SessionId, UnixTimestamp};

use crate::SessionTokenDigest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCredentialRecord {
    digest: SessionTokenDigest,
    session_id: SessionId,
    issued_at: UnixTimestamp,
}

impl SessionCredentialRecord {
    pub fn new(
        digest: SessionTokenDigest,
        session_id: SessionId,
        issued_at: UnixTimestamp,
    ) -> Self {
        Self {
            digest,
            session_id,
            issued_at,
        }
    }

    pub fn digest(&self) -> &SessionTokenDigest {
        &self.digest
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn issued_at(&self) -> UnixTimestamp {
        self.issued_at
    }
}

pub trait SessionCredentialRepository {
    type Error: Error + Send + Sync + 'static;

    fn insert(&self, credential: &SessionCredentialRecord) -> Result<(), Self::Error>;

    fn resolve(&self, digest: &SessionTokenDigest) -> Result<Option<SessionId>, Self::Error>;

    fn revoke_session(
        &self,
        session_id: &SessionId,
        revoked_at: UnixTimestamp,
    ) -> Result<usize, Self::Error>;
}
