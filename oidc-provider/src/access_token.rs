//! Opaque access tokens carry no account data to the browser or app. Only the
//! SHA-256 digest is persisted; UserInfo resolves and revalidates the grant.

use std::error::Error;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use getrandom::fill;
use rumahl_core::{OidcScope, SessionId, UnixTimestamp, UserId};
use sha2::{Digest, Sha256};

use crate::{OidcClientId, PairwiseSubject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessTokenGrantError {
    Randomness,
    InvalidLifetime,
    InvalidScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokenGrant {
    digest: [u8; 32],
    client_id: OidcClientId,
    user_id: UserId,
    session_id: SessionId,
    subject: PairwiseSubject,
    scopes: Vec<OidcScope>,
    expires_at: UnixTimestamp,
}

impl AccessTokenGrant {
    pub fn issue(
        client_id: OidcClientId,
        user_id: UserId,
        session_id: SessionId,
        subject: PairwiseSubject,
        scopes: Vec<OidcScope>,
        expires_at: UnixTimestamp,
        now: UnixTimestamp,
    ) -> Result<(Self, String), AccessTokenGrantError> {
        if now >= expires_at {
            return Err(AccessTokenGrantError::InvalidLifetime);
        }
        if subject.user_id() != user_id
            || !scopes.contains(&OidcScope::OpenId)
            || scopes
                .iter()
                .enumerate()
                .any(|(index, scope)| scopes[..index].contains(scope))
        {
            return Err(AccessTokenGrantError::InvalidScope);
        }
        let mut random = [0_u8; 32];
        fill(&mut random).map_err(|_| AccessTokenGrantError::Randomness)?;
        let raw = URL_SAFE_NO_PAD.encode(random);
        Ok((
            Self {
                digest: Self::digest_raw(&raw),
                client_id,
                user_id,
                session_id,
                subject,
                scopes,
                expires_at,
            },
            raw,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        digest: [u8; 32],
        client_id: OidcClientId,
        user_id: UserId,
        session_id: SessionId,
        subject: PairwiseSubject,
        scopes: Vec<OidcScope>,
        expires_at: UnixTimestamp,
    ) -> Result<Self, AccessTokenGrantError> {
        if subject.user_id() != user_id
            || !scopes.contains(&OidcScope::OpenId)
            || scopes
                .iter()
                .enumerate()
                .any(|(index, scope)| scopes[..index].contains(scope))
        {
            return Err(AccessTokenGrantError::InvalidScope);
        }
        Ok(Self {
            digest,
            client_id,
            user_id,
            session_id,
            subject,
            scopes,
            expires_at,
        })
    }

    pub fn digest_raw(raw: &str) -> [u8; 32] {
        Sha256::digest(raw.as_bytes()).into()
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub fn subject(&self) -> &PairwiseSubject {
        &self.subject
    }
    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }
    pub fn expires_at(&self) -> UnixTimestamp {
        self.expires_at
    }
}

pub trait OidcAccessTokenStore: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    fn insert_access(
        &self,
        grant: &AccessTokenGrant,
        family_id: Option<&str>,
    ) -> Result<(), Self::Error>;
    fn resolve_access(
        &self,
        raw: &str,
        now: UnixTimestamp,
    ) -> Result<Option<AccessTokenGrant>, Self::Error>;
    fn revoke_access(&self, raw: &str, now: UnixTimestamp) -> Result<usize, Self::Error>;
}
