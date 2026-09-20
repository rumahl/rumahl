use std::error::Error;
use std::fmt;

use rumahl_core::{AppId, InstallationId, OidcClientType, OidcScope, UnixTimestamp};

use crate::{OidcClientId, OidcClientSecret, OidcClientSecretDigest, OidcRedirectUri};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcTokenEndpointAuthMethod {
    None,
    ClientSecretBasic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcCodeChallengeMethod {
    S256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcClientRecord {
    client_id: OidcClientId,
    installation_id: InstallationId,
    app_id: AppId,
    display_name: String,
    client_type: OidcClientType,
    redirect_uri: OidcRedirectUri,
    scopes: Vec<OidcScope>,
    client_secret_digest: Option<OidcClientSecretDigest>,
    created_at: UnixTimestamp,
    revoked_at: Option<UnixTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcClientRecordError {
    EmptyDisplayName,
    DisplayNameTooLong,
    DisplayNameContainsControlCharacter,
    MissingOpenIdScope,
    DuplicateScope(OidcScope),
    PublicClientHasSecret,
    ConfidentialClientMissingSecret,
    RevokedBeforeCreation,
}

impl OidcClientRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        client_id: OidcClientId,
        installation_id: InstallationId,
        app_id: AppId,
        display_name: impl Into<String>,
        client_type: OidcClientType,
        redirect_uri: OidcRedirectUri,
        scopes: Vec<OidcScope>,
        client_secret_digest: Option<OidcClientSecretDigest>,
        created_at: UnixTimestamp,
        revoked_at: Option<UnixTimestamp>,
    ) -> Result<Self, OidcClientRecordError> {
        let display_name = display_name.into();
        let display_name = display_name.trim();

        if display_name.is_empty() {
            return Err(OidcClientRecordError::EmptyDisplayName);
        }

        if display_name.chars().count() > 120 {
            return Err(OidcClientRecordError::DisplayNameTooLong);
        }

        if display_name.chars().any(char::is_control) {
            return Err(OidcClientRecordError::DisplayNameContainsControlCharacter);
        }

        if !scopes.contains(&OidcScope::OpenId) {
            return Err(OidcClientRecordError::MissingOpenIdScope);
        }

        for (index, scope) in scopes.iter().enumerate() {
            if scopes[..index].contains(scope) {
                return Err(OidcClientRecordError::DuplicateScope(*scope));
            }
        }

        match (client_type, client_secret_digest) {
            (OidcClientType::Public, Some(_)) => {
                return Err(OidcClientRecordError::PublicClientHasSecret);
            }
            (OidcClientType::Confidential, None) => {
                return Err(OidcClientRecordError::ConfidentialClientMissingSecret);
            }
            _ => {}
        }

        if revoked_at.is_some_and(|revoked_at| revoked_at < created_at) {
            return Err(OidcClientRecordError::RevokedBeforeCreation);
        }

        Ok(Self {
            client_id,
            installation_id,
            app_id,
            display_name: display_name.to_owned(),
            client_type,
            redirect_uri,
            scopes,
            client_secret_digest,
            created_at,
            revoked_at,
        })
    }

    pub fn client_id(&self) -> &OidcClientId {
        &self.client_id
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn app_id(&self) -> &AppId {
        &self.app_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn client_type(&self) -> OidcClientType {
        self.client_type
    }

    pub fn redirect_uri(&self) -> &OidcRedirectUri {
        &self.redirect_uri
    }

    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }

    pub fn client_secret_digest(&self) -> Option<&OidcClientSecretDigest> {
        self.client_secret_digest.as_ref()
    }

    pub fn created_at(&self) -> UnixTimestamp {
        self.created_at
    }

    pub fn revoked_at(&self) -> Option<UnixTimestamp> {
        self.revoked_at
    }

    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }

    pub fn token_endpoint_auth_method(&self) -> OidcTokenEndpointAuthMethod {
        match self.client_type {
            OidcClientType::Public => OidcTokenEndpointAuthMethod::None,
            OidcClientType::Confidential => OidcTokenEndpointAuthMethod::ClientSecretBasic,
        }
    }

    pub fn code_challenge_method(&self) -> OidcCodeChallengeMethod {
        OidcCodeChallengeMethod::S256
    }

    pub fn matches_redirect_uri(&self, candidate: &str) -> bool {
        self.is_active() && self.redirect_uri.exactly_matches(candidate)
    }

    pub fn verifies_client_secret(&self, encoded_secret: &str) -> bool {
        let (Some(expected), Ok(supplied)) = (
            self.client_secret_digest,
            OidcClientSecret::parse(encoded_secret),
        ) else {
            return false;
        };

        expected.verifies(&supplied)
    }
}

impl fmt::Display for OidcClientRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDisplayName => write!(f, "OIDC client display name must not be empty"),
            Self::DisplayNameTooLong => write!(f, "OIDC client display name is too long"),
            Self::DisplayNameContainsControlCharacter => {
                write!(f, "OIDC client display name contains a control character")
            }
            Self::MissingOpenIdScope => write!(f, "OIDC client requires the openid scope"),
            Self::DuplicateScope(scope) => {
                write!(
                    f,
                    "OIDC client contains duplicate scope '{}'",
                    scope.as_str()
                )
            }
            Self::PublicClientHasSecret => write!(f, "public OIDC client must not have a secret"),
            Self::ConfidentialClientMissingSecret => {
                write!(f, "confidential OIDC client requires a secret digest")
            }
            Self::RevokedBeforeCreation => {
                write!(f, "OIDC client cannot be revoked before it was created")
            }
        }
    }
}

impl Error for OidcClientRecordError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn restore(
        client_type: OidcClientType,
        secret: Option<OidcClientSecretDigest>,
        revoked_at: Option<UnixTimestamp>,
    ) -> Result<OidcClientRecord, OidcClientRecordError> {
        OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.notes").unwrap(),
            "Notes",
            client_type,
            OidcRedirectUri::parse("https://notes.rumahl.local/oidc/callback").unwrap(),
            vec![OidcScope::OpenId, OidcScope::Profile],
            secret,
            UnixTimestamp::from_seconds(100),
            revoked_at,
        )
    }

    #[test]
    fn client_type_and_secret_presence_must_agree() {
        let secret = OidcClientSecret::generate().unwrap();

        assert_eq!(
            restore(OidcClientType::Public, Some(secret.digest()), None).unwrap_err(),
            OidcClientRecordError::PublicClientHasSecret
        );
        assert_eq!(
            restore(OidcClientType::Confidential, None, None).unwrap_err(),
            OidcClientRecordError::ConfidentialClientMissingSecret
        );
    }

    #[test]
    fn revoked_client_no_longer_matches_redirect_uri() {
        let client = restore(
            OidcClientType::Public,
            None,
            Some(UnixTimestamp::from_seconds(200)),
        )
        .unwrap();

        assert!(!client.matches_redirect_uri("https://notes.rumahl.local/oidc/callback"));
    }
}
