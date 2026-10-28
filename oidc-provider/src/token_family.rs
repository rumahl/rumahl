//! A refresh-token family belongs to exactly one user login and client.
//! Repositories must persist a rotation and the spent-token marker atomically.

use std::error::Error;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use getrandom::fill;
use rumahl_core::{SessionId, UnixTimestamp, UserId};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::OidcClientId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTokenFamilyError {
    Randomness,
    InvalidLifetime,
    Expired,
    Revoked,
    WrongToken,
    GenerationOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTokenFamily {
    id: String,
    user_id: UserId,
    session_id: SessionId,
    client_id: OidcClientId,
    current_digest: [u8; 32],
    generation: u64,
    expires_at: UnixTimestamp,
    revoked_at: Option<UnixTimestamp>,
}

impl RefreshTokenFamily {
    pub fn digest_raw(value: &str) -> [u8; 32] {
        digest(value)
    }
    pub fn create(
        user_id: UserId,
        session_id: SessionId,
        client_id: OidcClientId,
        expires_at: UnixTimestamp,
        now: UnixTimestamp,
    ) -> Result<(Self, String), RefreshTokenFamilyError> {
        if now >= expires_at {
            return Err(RefreshTokenFamilyError::InvalidLifetime);
        }
        let id = random_value()?;
        let raw = random_value()?;
        Ok((
            Self {
                id,
                user_id,
                session_id,
                client_id,
                current_digest: digest(&raw),
                generation: 0,
                expires_at,
                revoked_at: None,
            },
            raw,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: String,
        user_id: UserId,
        session_id: SessionId,
        client_id: OidcClientId,
        current_digest: [u8; 32],
        generation: u64,
        expires_at: UnixTimestamp,
        revoked_at: Option<UnixTimestamp>,
    ) -> Result<Self, RefreshTokenFamilyError> {
        if id.len() != 43
            || URL_SAFE_NO_PAD
                .decode(&id)
                .map_or(true, |bytes| bytes.len() != 32)
        {
            return Err(RefreshTokenFamilyError::InvalidLifetime);
        }
        Ok(Self {
            id,
            user_id,
            session_id,
            client_id,
            current_digest,
            generation,
            expires_at,
            revoked_at,
        })
    }

    /// Call only after a repository matched this token to the family's current
    /// digest. A spent digest is handled through `revoke_for_replay` instead.
    pub fn rotate(
        &mut self,
        presented: &str,
        now: UnixTimestamp,
    ) -> Result<String, RefreshTokenFamilyError> {
        if self.revoked_at.is_some() {
            return Err(RefreshTokenFamilyError::Revoked);
        }
        if now >= self.expires_at {
            return Err(RefreshTokenFamilyError::Expired);
        }
        if !bool::from(digest(presented).ct_eq(&self.current_digest)) {
            return Err(RefreshTokenFamilyError::WrongToken);
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(RefreshTokenFamilyError::GenerationOverflow)?;
        let raw = random_value()?;
        self.current_digest = digest(&raw);
        self.generation = next_generation;
        Ok(raw)
    }

    /// Reuse of any previously spent token kills the entire family.
    pub fn revoke_for_replay(&mut self, now: UnixTimestamp) {
        self.revoked_at.get_or_insert(now);
    }

    pub fn revoke(&mut self, now: UnixTimestamp) {
        self.revoked_at.get_or_insert(now);
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }
    pub fn current_digest(&self) -> &[u8; 32] {
        &self.current_digest
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn expires_at(&self) -> UnixTimestamp {
        self.expires_at
    }
    pub fn revoked_at(&self) -> Option<UnixTimestamp> {
        self.revoked_at
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum RefreshRotation {
    Rotated(String),
    ReplayRevoked,
    Invalid,
}

impl std::fmt::Debug for RefreshRotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rotated(_) => f.write_str("Rotated(<redacted>)"),
            Self::ReplayRevoked => f.write_str("ReplayRevoked"),
            Self::Invalid => f.write_str("Invalid"),
        }
    }
}

pub trait OidcRefreshTokenStore: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn insert_family(&self, family: &RefreshTokenFamily) -> Result<(), Self::Error>;
    fn rotate(&self, raw: &str, now: UnixTimestamp) -> Result<RefreshRotation, Self::Error>;
    fn revoke_for_session(
        &self,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<usize, Self::Error>;
}

pub fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn random_value() -> Result<String, RefreshTokenFamilyError> {
    let mut random = [0_u8; 32];
    fill(&mut random).map_err(|_| RefreshTokenFamilyError::Randomness)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_once_and_revokes_on_replay() {
        let (mut family, first) = RefreshTokenFamily::create(
            UserId::new(),
            SessionId::new(),
            OidcClientId::generate().unwrap(),
            UnixTimestamp::from_seconds(200),
            UnixTimestamp::from_seconds(100),
        )
        .unwrap();
        let old_digest = *family.current_digest();
        let next = family
            .rotate(&first, UnixTimestamp::from_seconds(110))
            .unwrap();
        assert_ne!(old_digest, *family.current_digest());
        assert_eq!(family.generation(), 1);
        assert_eq!(
            family.rotate(&first, UnixTimestamp::from_seconds(111)),
            Err(RefreshTokenFamilyError::WrongToken)
        );
        family.revoke_for_replay(UnixTimestamp::from_seconds(111));
        assert_eq!(
            family.rotate(&next, UnixTimestamp::from_seconds(112)),
            Err(RefreshTokenFamilyError::Revoked)
        );
    }

    #[test]
    fn expired_or_revoked_family_cannot_rotate() {
        let (mut family, token) = RefreshTokenFamily::create(
            UserId::new(),
            SessionId::new(),
            OidcClientId::generate().unwrap(),
            UnixTimestamp::from_seconds(200),
            UnixTimestamp::from_seconds(100),
        )
        .unwrap();
        assert_eq!(
            family.rotate(&token, UnixTimestamp::from_seconds(200)),
            Err(RefreshTokenFamilyError::Expired)
        );
        family.revoke(UnixTimestamp::from_seconds(150));
        assert_eq!(
            family.rotate(&token, UnixTimestamp::from_seconds(151)),
            Err(RefreshTokenFamilyError::Revoked)
        );
    }
}
