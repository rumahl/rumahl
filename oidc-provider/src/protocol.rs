//! Transport-neutral Authorization Code flow. HTTP adapters must authenticate
//! the browser session before calling `begin` or `approve`, and must never use
//! client-supplied account or session identifiers.

use std::error::Error;

use rumahl_core::{OidcScope, SessionId, UnixTimestamp, UserId};
use serde_json::{Value, json};

use crate::{
    AccessTokenGrant, AuthorizationTransaction, OidcAccessTokenStore, OidcAuthorizationStore,
    OidcClientId, OidcClientRecord, OidcClientRepository, OidcSigningKey, PairwiseSubject,
    PkceChallenge, UserConsent,
};

const TOKEN_LIFETIME_SECONDS: u64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcProtocolError {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    AccessDenied,
    UnsupportedScope,
    Unavailable,
}

/// The OS account/session authority, not the app or OIDC database, decides
/// whether a local login is still valid and which public claims are exposed.
pub trait OidcUserSource: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn active_authentication_time(
        &self,
        user_id: UserId,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<Option<UnixTimestamp>, Self::Error>;

    fn public_claims(&self, user_id: UserId) -> Result<Option<OidcPublicClaims>, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcPublicClaims {
    pub name: Option<String>,
    pub email: Option<String>,
    pub email_verified: bool,
}

pub struct OidcTokenResponse {
    access_token: String,
    id_token: String,
    expires_in: u64,
    scope: String,
}

impl std::fmt::Debug for OidcTokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcTokenResponse")
            .field("access_token", &"<redacted>")
            .field("id_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish()
    }
}

impl OidcTokenResponse {
    pub fn as_json(&self) -> Value {
        json!({
            "access_token": self.access_token,
            "token_type": "Bearer",
            "expires_in": self.expires_in,
            "id_token": self.id_token,
            "scope": self.scope,
        })
    }
}

pub struct OidcProtocol<C, A, T, U> {
    issuer: String,
    clients: C,
    authorizations: A,
    access_tokens: T,
    users: U,
    signing_key: OidcSigningKey,
}

impl<C, A, T, U> OidcProtocol<C, A, T, U>
where
    C: OidcClientRepository + Send + Sync + 'static,
    A: OidcAuthorizationStore,
    T: OidcAccessTokenStore,
    U: OidcUserSource,
{
    pub fn new(
        issuer: &str,
        clients: C,
        authorizations: A,
        access_tokens: T,
        users: U,
        signing_key: OidcSigningKey,
    ) -> Result<Self, OidcProtocolError> {
        let url = url::Url::parse(issuer).map_err(|_| OidcProtocolError::InvalidRequest)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.username() != ""
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.origin().ascii_serialization() != issuer
        {
            return Err(OidcProtocolError::InvalidRequest);
        }
        Ok(Self {
            issuer: issuer.to_owned(),
            clients,
            authorizations,
            access_tokens,
            users,
            signing_key,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        &self,
        client_id: &str,
        redirect_uri: &str,
        scopes: Vec<OidcScope>,
        state: &str,
        nonce: &str,
        code_challenge: &str,
        user_id: UserId,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<String, OidcProtocolError> {
        if scopes.contains(&OidcScope::OfflineAccess) {
            return Err(OidcProtocolError::UnsupportedScope);
        }
        self.require_live_session(user_id, session_id, now)?;
        let client = self.active_client(client_id)?;
        let challenge =
            PkceChallenge::parse(code_challenge).map_err(|_| OidcProtocolError::InvalidRequest)?;
        let transaction = AuthorizationTransaction::start(
            &client,
            redirect_uri,
            user_id,
            session_id,
            scopes,
            state,
            nonce,
            challenge,
            now,
        )
        .map_err(|_| OidcProtocolError::InvalidRequest)?;
        self.authorizations
            .insert_transaction(&transaction)
            .map_err(|_| OidcProtocolError::Unavailable)?;
        Ok(transaction.id().to_owned())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn approve(
        &self,
        transaction_id: &str,
        client_id: &str,
        scopes: Vec<OidcScope>,
        user_id: UserId,
        session_id: SessionId,
        now: UnixTimestamp,
    ) -> Result<crate::ApprovedAuthorization, OidcProtocolError> {
        self.require_live_session(user_id, session_id, now)?;
        let client = self.active_client(client_id)?;
        let consent = UserConsent::new(user_id, &client, scopes, false)
            .map_err(|_| OidcProtocolError::InvalidRequest)?;
        self.authorizations
            .approve_transaction(transaction_id, &client, user_id, session_id, &consent, now)
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::AccessDenied)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn exchange_code(
        &self,
        client_id: &str,
        client_secret: Option<&str>,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<OidcTokenResponse, OidcProtocolError> {
        let client = self.active_client(client_id)?;
        authenticate_client(&client, client_secret)?;
        let code = self
            .authorizations
            .consume_code(code, &client, redirect_uri, verifier, now)
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::InvalidGrant)?;
        let auth_time = self.require_live_session(code.user_id(), code.session_id(), now)?;
        let subject = self
            .authorizations
            .find_subject(code.user_id(), *code.installation_id())
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::Unavailable)?;
        let profile = self
            .users
            .public_claims(code.user_id())
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::InvalidGrant)?;
        self.issue_tokens(&client, &code, subject, &profile, auth_time, now)
    }

    pub fn userinfo(
        &self,
        raw_access_token: &str,
        now: UnixTimestamp,
    ) -> Result<Value, OidcProtocolError> {
        let grant = self
            .access_tokens
            .resolve_access(raw_access_token, now)
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::InvalidGrant)?;
        self.require_live_session(grant.user_id(), grant.session_id(), now)?;
        let profile = self
            .users
            .public_claims(grant.user_id())
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::InvalidGrant)?;
        let mut body = json!({"sub": grant.subject().value()});
        if grant.scopes().contains(&OidcScope::Profile)
            && let Some(name) = profile.name
        {
            body["name"] = json!(name);
        }
        if grant.scopes().contains(&OidcScope::Email)
            && let Some(email) = profile.email
        {
            body["email"] = json!(email);
            body["email_verified"] = json!(profile.email_verified);
        }
        Ok(body)
    }

    pub fn revoke_access(&self, raw: &str, now: UnixTimestamp) -> Result<(), OidcProtocolError> {
        self.access_tokens
            .revoke_access(raw, now)
            .map_err(|_| OidcProtocolError::Unavailable)?;
        Ok(())
    }

    pub fn discovery(&self) -> Value {
        json!({
            "issuer": self.issuer,
            "authorization_endpoint": format!("{}/oauth2/authorize", self.issuer),
            "token_endpoint": format!("{}/oauth2/token", self.issuer),
            "userinfo_endpoint": format!("{}/oauth2/userinfo", self.issuer),
            "jwks_uri": format!("{}/oauth2/jwks", self.issuer),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "subject_types_supported": ["pairwise"],
            "id_token_signing_alg_values_supported": ["EdDSA"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["client_secret_basic", "none"],
            "scopes_supported": ["openid", "profile", "email"],
            "claims_supported": ["sub", "name", "email", "email_verified", "auth_time", "nonce"],
        })
    }

    pub fn jwks(&self) -> Value {
        json!({"keys": [self.signing_key.public_jwk()]})
    }

    pub fn client_display_name(&self, client_id: &str) -> Result<String, OidcProtocolError> {
        Ok(self.active_client(client_id)?.display_name().to_owned())
    }

    fn active_client(&self, raw: &str) -> Result<OidcClientRecord, OidcProtocolError> {
        let id = OidcClientId::parse(raw).map_err(|_| OidcProtocolError::InvalidClient)?;
        self.clients
            .find_active_by_id(&id)
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::InvalidClient)
    }

    fn require_live_session(
        &self,
        user: UserId,
        session: SessionId,
        now: UnixTimestamp,
    ) -> Result<UnixTimestamp, OidcProtocolError> {
        self.users
            .active_authentication_time(user, session, now)
            .map_err(|_| OidcProtocolError::Unavailable)?
            .ok_or(OidcProtocolError::AccessDenied)
    }

    fn issue_tokens(
        &self,
        client: &OidcClientRecord,
        code: &crate::AuthorizationCode,
        subject: PairwiseSubject,
        profile: &OidcPublicClaims,
        auth_time: UnixTimestamp,
        now: UnixTimestamp,
    ) -> Result<OidcTokenResponse, OidcProtocolError> {
        let expires_at = now
            .as_seconds()
            .checked_add(TOKEN_LIFETIME_SECONDS)
            .map(UnixTimestamp::from_seconds)
            .ok_or(OidcProtocolError::Unavailable)?;
        let id_token = self
            .signing_key
            .sign_id_token(
                &self.issuer,
                &subject,
                client.client_id(),
                code.nonce(),
                auth_time,
                now,
                expires_at,
                code.scopes()
                    .contains(&OidcScope::Profile)
                    .then_some(profile.name.as_deref())
                    .flatten(),
            )
            .map_err(|_| OidcProtocolError::Unavailable)?;
        let (grant, access_token) = AccessTokenGrant::issue(
            client.client_id().clone(),
            code.user_id(),
            code.session_id(),
            subject,
            code.scopes().to_vec(),
            expires_at,
            now,
        )
        .map_err(|_| OidcProtocolError::Unavailable)?;
        self.access_tokens
            .insert_access(&grant, None)
            .map_err(|_| OidcProtocolError::Unavailable)?;
        Ok(OidcTokenResponse {
            access_token,
            id_token,
            expires_in: TOKEN_LIFETIME_SECONDS,
            scope: code
                .scopes()
                .iter()
                .map(|scope| scope.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        })
    }
}

fn authenticate_client(
    client: &OidcClientRecord,
    supplied: Option<&str>,
) -> Result<(), OidcProtocolError> {
    match (client.token_endpoint_auth_method(), supplied) {
        (crate::OidcTokenEndpointAuthMethod::None, None) => Ok(()),
        (crate::OidcTokenEndpointAuthMethod::ClientSecretBasic, Some(secret))
            if client.verifies_client_secret(secret) =>
        {
            Ok(())
        }
        _ => Err(OidcProtocolError::InvalidClient),
    }
}
