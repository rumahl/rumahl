//! Authorization state is bound to one authenticated OS login and one active
//! installation. No app-supplied value can select a different redirect host.

use std::error::Error;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use getrandom::fill;
use rumahl_core::{InstallationId, OidcScope, SessionId, UnixTimestamp, UserId};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{OidcClientId, OidcClientRecord, OidcRedirectUri};

const TRANSACTION_LIFETIME_SECONDS: u64 = 300;
const CODE_LIFETIME_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcAuthorizationError {
    InactiveClient,
    RedirectMismatch,
    InvalidState,
    InvalidNonce,
    InvalidPkceChallenge,
    InvalidPkceVerifier,
    ScopeNotRegistered,
    MissingOpenIdScope,
    OfflineAccessRequiresConsent,
    Expired,
    WrongLogin,
    WrongClient,
    Randomness,
    InvalidLifetime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceChallenge(String);

impl PkceChallenge {
    /// Only the RFC 7636 S256 output is accepted; `plain` has no fallback.
    pub fn parse(value: &str) -> Result<Self, OidcAuthorizationError> {
        if value.len() != 43
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || URL_SAFE_NO_PAD
                .decode(value)
                .map_or(true, |bytes| bytes.len() != 32)
        {
            return Err(OidcAuthorizationError::InvalidPkceChallenge);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn verify(&self, verifier: &str) -> bool {
        if !(43..=128).contains(&verifier.len())
            || !verifier.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
            })
        {
            return false;
        }
        let digest = Sha256::digest(verifier.as_bytes());
        let candidate = URL_SAFE_NO_PAD.encode(digest);
        candidate.as_bytes().ct_eq(self.0.as_bytes()).into()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationTransaction {
    id: String,
    client_id: OidcClientId,
    installation_id: InstallationId,
    redirect_uri: OidcRedirectUri,
    user_id: UserId,
    session_id: SessionId,
    scopes: Vec<OidcScope>,
    state: String,
    nonce: String,
    challenge: PkceChallenge,
    expires_at: UnixTimestamp,
}

impl AuthorizationTransaction {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: String,
        client: &OidcClientRecord,
        user_id: UserId,
        session_id: SessionId,
        scopes: Vec<OidcScope>,
        state: String,
        nonce: String,
        challenge: PkceChallenge,
        expires_at: UnixTimestamp,
    ) -> Result<Self, OidcAuthorizationError> {
        if !valid_random_id(&id)
            || !client.is_active()
            || !valid_opaque(&state)
            || !valid_opaque(&nonce)
            || !valid_scopes(client, &scopes)
        {
            return Err(OidcAuthorizationError::InvalidState);
        }
        Ok(Self {
            id,
            client_id: client.client_id().clone(),
            installation_id: *client.installation_id(),
            redirect_uri: client.redirect_uri().clone(),
            user_id,
            session_id,
            scopes,
            state,
            nonce,
            challenge,
            expires_at,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        client: &OidcClientRecord,
        redirect_uri: &str,
        user_id: UserId,
        session_id: SessionId,
        scopes: Vec<OidcScope>,
        state: &str,
        nonce: &str,
        challenge: PkceChallenge,
        now: UnixTimestamp,
    ) -> Result<Self, OidcAuthorizationError> {
        if !client.is_active() {
            return Err(OidcAuthorizationError::InactiveClient);
        }
        if !client.matches_redirect_uri(redirect_uri) {
            return Err(OidcAuthorizationError::RedirectMismatch);
        }
        if !valid_opaque(state) {
            return Err(OidcAuthorizationError::InvalidState);
        }
        if !valid_opaque(nonce) {
            return Err(OidcAuthorizationError::InvalidNonce);
        }
        if !scopes.contains(&OidcScope::OpenId) {
            return Err(OidcAuthorizationError::MissingOpenIdScope);
        }
        if !valid_scopes(client, &scopes) {
            return Err(OidcAuthorizationError::ScopeNotRegistered);
        }
        let expires_at = now
            .as_seconds()
            .checked_add(TRANSACTION_LIFETIME_SECONDS)
            .ok_or(OidcAuthorizationError::InvalidLifetime)?;
        Ok(Self {
            id: random_value()?,
            client_id: client.client_id().clone(),
            installation_id: *client.installation_id(),
            redirect_uri: client.redirect_uri().clone(),
            user_id,
            session_id,
            scopes,
            state: state.to_owned(),
            nonce: nonce.to_owned(),
            challenge,
            expires_at: UnixTimestamp::from_seconds(expires_at),
        })
    }

    pub fn approve(
        &self,
        client: &OidcClientRecord,
        consent: &UserConsent,
        user_id: UserId,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<(AuthorizationCode, String), OidcAuthorizationError> {
        if now >= self.expires_at {
            return Err(OidcAuthorizationError::Expired);
        }
        if !client.is_active()
            || client.client_id() != &self.client_id
            || client.installation_id() != &self.installation_id
            || client.redirect_uri() != &self.redirect_uri
        {
            return Err(OidcAuthorizationError::InactiveClient);
        }
        if self.user_id != user_id || self.session_id != session_id {
            return Err(OidcAuthorizationError::WrongLogin);
        }
        if consent.user_id != user_id || consent.client_id != self.client_id {
            return Err(OidcAuthorizationError::WrongClient);
        }
        if !self
            .scopes
            .iter()
            .all(|scope| consent.scopes.contains(scope))
        {
            return Err(OidcAuthorizationError::ScopeNotRegistered);
        }
        if self.scopes.contains(&OidcScope::OfflineAccess) && !consent.explicit_offline_access {
            return Err(OidcAuthorizationError::OfflineAccessRequiresConsent);
        }
        let expires_at = now
            .as_seconds()
            .checked_add(CODE_LIFETIME_SECONDS)
            .ok_or(OidcAuthorizationError::InvalidLifetime)?;
        let raw = random_value()?;
        Ok((
            AuthorizationCode {
                digest: Sha256::digest(raw.as_bytes()).into(),
                client_id: self.client_id.clone(),
                installation_id: self.installation_id,
                redirect_uri: self.redirect_uri.clone(),
                user_id,
                session_id,
                scopes: self.scopes.clone(),
                nonce: self.nonce.clone(),
                challenge: self.challenge.clone(),
                expires_at: UnixTimestamp::from_seconds(expires_at),
            },
            raw,
        ))
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }
    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }
    pub fn redirect_uri(&self) -> &OidcRedirectUri {
        &self.redirect_uri
    }
    pub fn state(&self) -> &str {
        &self.state
    }
    pub fn expires_at(&self) -> UnixTimestamp {
        self.expires_at
    }
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
    pub fn challenge(&self) -> &PkceChallenge {
        &self.challenge
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationCode {
    digest: [u8; 32],
    client_id: OidcClientId,
    installation_id: InstallationId,
    redirect_uri: OidcRedirectUri,
    user_id: UserId,
    session_id: SessionId,
    scopes: Vec<OidcScope>,
    nonce: String,
    challenge: PkceChallenge,
    expires_at: UnixTimestamp,
}

impl AuthorizationCode {
    pub fn digest_raw(raw: &str) -> [u8; 32] {
        Sha256::digest(raw.as_bytes()).into()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        digest: [u8; 32],
        client: &OidcClientRecord,
        user_id: UserId,
        session_id: SessionId,
        scopes: Vec<OidcScope>,
        nonce: String,
        challenge: PkceChallenge,
        expires_at: UnixTimestamp,
    ) -> Result<Self, OidcAuthorizationError> {
        if !client.is_active() || !valid_scopes(client, &scopes) || !valid_opaque(&nonce) {
            return Err(OidcAuthorizationError::InvalidState);
        }
        Ok(Self {
            digest,
            client_id: client.client_id().clone(),
            installation_id: *client.installation_id(),
            redirect_uri: client.redirect_uri().clone(),
            user_id,
            session_id,
            scopes,
            nonce,
            challenge,
            expires_at,
        })
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }
    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
    pub fn expires_at(&self) -> UnixTimestamp {
        self.expires_at
    }
    pub fn redirect_uri(&self) -> &OidcRedirectUri {
        &self.redirect_uri
    }
    pub fn challenge(&self) -> &PkceChallenge {
        &self.challenge
    }

    pub fn verify_exchange(
        &self,
        raw: &str,
        client: &OidcClientRecord,
        redirect_uri: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<(), OidcAuthorizationError> {
        if now >= self.expires_at {
            return Err(OidcAuthorizationError::Expired);
        }
        if !client.is_active()
            || client.client_id() != &self.client_id
            || client.installation_id() != &self.installation_id
        {
            return Err(OidcAuthorizationError::WrongClient);
        }
        if !self.redirect_uri.exactly_matches(redirect_uri) {
            return Err(OidcAuthorizationError::RedirectMismatch);
        }
        let supplied: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
        if !bool::from(supplied.ct_eq(&self.digest)) || !self.challenge.verify(verifier) {
            return Err(OidcAuthorizationError::InvalidPkceVerifier);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConsent {
    user_id: UserId,
    client_id: OidcClientId,
    scopes: Vec<OidcScope>,
    explicit_offline_access: bool,
}

impl UserConsent {
    pub fn new(
        user_id: UserId,
        client: &OidcClientRecord,
        scopes: Vec<OidcScope>,
        explicit_offline_access: bool,
    ) -> Result<Self, OidcAuthorizationError> {
        if !client.is_active() {
            return Err(OidcAuthorizationError::InactiveClient);
        }
        if !valid_scopes(client, &scopes) {
            return Err(OidcAuthorizationError::ScopeNotRegistered);
        }
        if scopes.contains(&OidcScope::OfflineAccess) && !explicit_offline_access {
            return Err(OidcAuthorizationError::OfflineAccessRequiresConsent);
        }
        Ok(Self {
            user_id,
            client_id: client.client_id().clone(),
            scopes,
            explicit_offline_access,
        })
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }
    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }
    pub fn explicit_offline_access(&self) -> bool {
        self.explicit_offline_access
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairwiseSubject {
    user_id: UserId,
    installation_id: InstallationId,
    value: String,
}

pub struct ApprovedAuthorization {
    code: String,
    state: String,
    subject: PairwiseSubject,
    redirect_uri: OidcRedirectUri,
}

impl ApprovedAuthorization {
    pub fn new(
        code: String,
        state: String,
        subject: PairwiseSubject,
        redirect_uri: OidcRedirectUri,
    ) -> Self {
        Self {
            code,
            state,
            subject,
            redirect_uri,
        }
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn state(&self) -> &str {
        &self.state
    }
    pub fn subject(&self) -> &PairwiseSubject {
        &self.subject
    }
    pub fn redirect_uri(&self) -> &OidcRedirectUri {
        &self.redirect_uri
    }
}

pub trait OidcAuthorizationStore: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn insert_transaction(&self, transaction: &AuthorizationTransaction)
    -> Result<(), Self::Error>;

    /// Atomically consumes the transaction, records consent and a one-use code,
    /// and returns the installation-stable pairwise subject.
    fn approve_transaction(
        &self,
        transaction_id: &str,
        client: &OidcClientRecord,
        user_id: UserId,
        session_id: SessionId,
        consent: &UserConsent,
        now: UnixTimestamp,
    ) -> Result<Option<ApprovedAuthorization>, Self::Error>;

    /// Validates and consumes a code in one durable transaction. `None` is the
    /// generic public failure for unknown, expired, replayed or malformed code.
    fn consume_code(
        &self,
        raw_code: &str,
        client: &OidcClientRecord,
        redirect_uri: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<Option<AuthorizationCode>, Self::Error>;

    /// Resolve only a subject established by an approved authorization. The
    /// subject never comes from a client-supplied token or redirect parameter.
    fn find_subject(
        &self,
        user_id: UserId,
        installation_id: InstallationId,
    ) -> Result<Option<PairwiseSubject>, Self::Error>;
}

impl PairwiseSubject {
    pub fn generate(
        user_id: UserId,
        installation_id: InstallationId,
    ) -> Result<Self, OidcAuthorizationError> {
        Ok(Self {
            user_id,
            installation_id,
            value: random_value()?,
        })
    }

    pub fn restore(
        user_id: UserId,
        installation_id: InstallationId,
        value: String,
    ) -> Result<Self, OidcAuthorizationError> {
        if value.len() != 43
            || URL_SAFE_NO_PAD
                .decode(&value)
                .map_or(true, |bytes| bytes.len() != 32)
        {
            return Err(OidcAuthorizationError::InvalidState);
        }
        Ok(Self {
            user_id,
            installation_id,
            value,
        })
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }
    pub fn installation_id(&self) -> InstallationId {
        self.installation_id
    }
    pub fn value(&self) -> &str {
        &self.value
    }
}

fn valid_opaque(value: &str) -> bool {
    (8..=512).contains(&value.len()) && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn valid_random_id(value: &str) -> bool {
    value.len() == 43
        && URL_SAFE_NO_PAD
            .decode(value)
            .is_ok_and(|bytes| bytes.len() == 32)
}

fn valid_scopes(client: &OidcClientRecord, scopes: &[OidcScope]) -> bool {
    scopes.contains(&OidcScope::OpenId)
        && scopes.len() <= client.scopes().len()
        && !scopes.iter().enumerate().any(|(index, scope)| {
            scopes[..index].contains(scope) || !client.scopes().contains(scope)
        })
}

fn random_value() -> Result<String, OidcAuthorizationError> {
    let mut random = [0_u8; 32];
    fill(&mut random).map_err(|_| OidcAuthorizationError::Randomness)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{AppId, OidcClientType};

    const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";

    fn client() -> OidcClientRecord {
        let secret = crate::OidcClientSecret::generate().unwrap();
        OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Confidential,
            OidcRedirectUri::parse("https://cloud.rumahl.dev/apps/oidc/callback").unwrap(),
            vec![
                OidcScope::OpenId,
                OidcScope::Profile,
                OidcScope::OfflineAccess,
            ],
            Some(secret.digest()),
            UnixTimestamp::from_seconds(100),
            None,
        )
        .unwrap()
    }

    fn challenge() -> PkceChallenge {
        PkceChallenge::parse(&URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()))).unwrap()
    }

    #[test]
    fn code_requires_exact_client_redirect_pkce_and_one_login() {
        let client = client();
        let user = UserId::new();
        let session = SessionId::new();
        let redirect = client.redirect_uri().as_str();
        let transaction = AuthorizationTransaction::start(
            &client,
            redirect,
            user,
            session,
            vec![OidcScope::OpenId, OidcScope::Profile],
            "opaque-state-123",
            "opaque-nonce-123",
            challenge(),
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
        let consent = UserConsent::new(
            user,
            &client,
            vec![OidcScope::OpenId, OidcScope::Profile],
            false,
        )
        .unwrap();
        assert_eq!(
            transaction
                .approve(
                    &client,
                    &consent,
                    user,
                    SessionId::new(),
                    UnixTimestamp::from_seconds(111)
                )
                .unwrap_err(),
            OidcAuthorizationError::WrongLogin
        );
        let (code, raw) = transaction
            .approve(
                &client,
                &consent,
                user,
                session,
                UnixTimestamp::from_seconds(111),
            )
            .unwrap();
        assert!(!format!("{code:?}").contains(&raw));
        assert_eq!(
            code.verify_exchange(
                &raw,
                &client,
                redirect,
                VERIFIER,
                UnixTimestamp::from_seconds(112)
            ),
            Ok(())
        );
        assert_eq!(
            code.verify_exchange(
                &raw,
                &client,
                redirect,
                "wrong",
                UnixTimestamp::from_seconds(112)
            ),
            Err(OidcAuthorizationError::InvalidPkceVerifier)
        );
        assert_eq!(
            code.verify_exchange(
                &raw,
                &client,
                &format!("{redirect}/"),
                VERIFIER,
                UnixTimestamp::from_seconds(112)
            ),
            Err(OidcAuthorizationError::RedirectMismatch)
        );
        assert_eq!(
            code.verify_exchange(
                &raw,
                &client,
                redirect,
                VERIFIER,
                UnixTimestamp::from_seconds(171)
            ),
            Err(OidcAuthorizationError::Expired)
        );
    }

    #[test]
    fn rejects_scope_escalation_and_requires_explicit_offline_consent() {
        let client = client();
        let user = UserId::new();
        let session = SessionId::new();
        let redirect = client.redirect_uri().as_str();
        let transaction = AuthorizationTransaction::start(
            &client,
            redirect,
            user,
            session,
            vec![OidcScope::OpenId, OidcScope::OfflineAccess],
            "opaque-state-123",
            "opaque-nonce-123",
            challenge(),
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
        assert_eq!(
            UserConsent::new(
                user,
                &client,
                vec![OidcScope::OpenId, OidcScope::OfflineAccess],
                false
            )
            .unwrap_err(),
            OidcAuthorizationError::OfflineAccessRequiresConsent
        );
        let consent = UserConsent::new(user, &client, vec![OidcScope::OpenId], false).unwrap();
        assert_eq!(
            transaction
                .approve(
                    &client,
                    &consent,
                    user,
                    session,
                    UnixTimestamp::from_seconds(111)
                )
                .unwrap_err(),
            OidcAuthorizationError::ScopeNotRegistered
        );
        assert_eq!(
            AuthorizationTransaction::start(
                &client,
                redirect,
                user,
                session,
                vec![OidcScope::Email, OidcScope::OpenId],
                "opaque-state-123",
                "opaque-nonce-123",
                challenge(),
                UnixTimestamp::from_seconds(110)
            )
            .unwrap_err(),
            OidcAuthorizationError::ScopeNotRegistered
        );
    }

    #[test]
    fn subjects_are_random_and_bound_to_user_and_installation() {
        let installation = InstallationId::new();
        let first = PairwiseSubject::generate(UserId::new(), installation).unwrap();
        let second = PairwiseSubject::generate(UserId::new(), installation).unwrap();
        assert_ne!(first.value(), second.value());
        assert_eq!(
            PairwiseSubject::restore(first.user_id(), installation, first.value().to_owned())
                .unwrap(),
            first
        );
    }
}
