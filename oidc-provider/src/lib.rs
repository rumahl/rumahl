//! Transport-neutral OpenID Connect provider domain and registration services.
//!
//! Installed rumahl OS apps may declare a logical OIDC callback in their
//! manifest. This crate turns that declaration into a concrete per-installation
//! client registration without allowing the manifest to choose an arbitrary
//! redirect host or reusable client secret.

mod app_lifecycle;
mod client;
mod client_id;
mod client_secret;
mod redirect_uri;
mod registrar;
mod repository;

pub use app_lifecycle::{
    OidcAppInstallError, OidcAppInstallResult, OidcAppLifecycle, OidcAppUninstallError,
    OidcAppUninstallResult,
};
pub use client::{
    OidcClientRecord, OidcClientRecordError, OidcCodeChallengeMethod, OidcTokenEndpointAuthMethod,
};
pub use client_id::{OidcClientId, OidcClientIdGenerationError, OidcClientIdParseError};
pub use client_secret::{
    OidcClientSecret, OidcClientSecretDigest, OidcClientSecretGenerationError,
    OidcClientSecretParseError,
};
pub use redirect_uri::{OidcRedirectUri, OidcRedirectUriError};
pub use registrar::{
    InstalledAppOriginResolver, OidcClientRegistrar, OidcClientRegistration,
    OidcClientRegistrationError,
};
pub use repository::OidcClientRepository;
