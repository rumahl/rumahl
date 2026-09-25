//! Transport-neutral OpenID Connect provider domain and registration services.
//!
//! Installed rumahl OS apps may declare a logical OIDC callback in their
//! manifest. This crate turns that declaration into a concrete per-installation
//! client registration without allowing the manifest to choose an arbitrary
//! redirect host or reusable client secret.

mod access_token;
mod authorization;
mod client;
mod client_id;
mod client_secret;
mod protocol;
mod redirect_uri;
mod registrar;
mod repository;
mod signing;
mod token_family;

pub use access_token::{AccessTokenGrant, AccessTokenGrantError, OidcAccessTokenStore};
pub use authorization::{
    ApprovedAuthorization, AuthorizationCode, AuthorizationTransaction, OidcAuthorizationError,
    OidcAuthorizationStore, PairwiseSubject, PkceChallenge, UserConsent,
};
pub use client::{
    OidcClientRecord, OidcClientRecordError, OidcCodeChallengeMethod, OidcTokenEndpointAuthMethod,
};
pub use client_id::{OidcClientId, OidcClientIdGenerationError, OidcClientIdParseError};
pub use client_secret::{
    OidcClientSecret, OidcClientSecretDigest, OidcClientSecretGenerationError,
    OidcClientSecretParseError,
};
pub use protocol::{
    OidcProtocol, OidcProtocolError, OidcPublicClaims, OidcTokenResponse, OidcUserSource,
};
pub use redirect_uri::{OidcRedirectUri, OidcRedirectUriError};
pub use registrar::{
    InstalledAppOriginResolver, OIDC_CLIENT_SECRET_PURPOSE, OidcClientProvisioningError,
    OidcClientProvisioningResult, OidcClientRegistrar, OidcClientRegistration,
};
pub use repository::OidcClientRepository;
pub use signing::{OidcSigningKey, OidcSigningKeyError};
pub use token_family::{
    OidcRefreshTokenStore, RefreshRotation, RefreshTokenFamily, RefreshTokenFamilyError,
};
