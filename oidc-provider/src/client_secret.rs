use std::error::Error;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const CLIENT_SECRET_LENGTH: usize = 32;

pub struct OidcClientSecret(Zeroizing<[u8; CLIENT_SECRET_LENGTH]>);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OidcClientSecretDigest([u8; 32]);

#[derive(Debug)]
pub struct OidcClientSecretGenerationError(getrandom::Error);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcClientSecretParseError {
    InvalidEncoding(base64::DecodeError),
    InvalidLength(usize),
}

impl OidcClientSecret {
    pub fn generate() -> Result<Self, OidcClientSecretGenerationError> {
        let mut bytes = Zeroizing::new([0_u8; CLIENT_SECRET_LENGTH]);
        getrandom::fill(&mut bytes[..]).map_err(OidcClientSecretGenerationError)?;

        Ok(Self(bytes))
    }

    pub fn parse(encoded: &str) -> Result<Self, OidcClientSecretParseError> {
        let decoded = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(OidcClientSecretParseError::InvalidEncoding)?,
        );

        if decoded.len() != CLIENT_SECRET_LENGTH {
            return Err(OidcClientSecretParseError::InvalidLength(decoded.len()));
        }

        let mut bytes = [0_u8; CLIENT_SECRET_LENGTH];
        bytes.copy_from_slice(&decoded);

        Ok(Self(Zeroizing::new(bytes)))
    }

    pub fn encode(&self) -> Zeroizing<String> {
        Zeroizing::new(URL_SAFE_NO_PAD.encode(&self.0[..]))
    }

    pub fn digest(&self) -> OidcClientSecretDigest {
        OidcClientSecretDigest(Sha256::digest(&self.0[..]).into())
    }
}

impl fmt::Debug for OidcClientSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcClientSecret").finish_non_exhaustive()
    }
}

impl OidcClientSecretDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn verifies(&self, secret: &OidcClientSecret) -> bool {
        bool::from(self.0.ct_eq(secret.digest().as_bytes()))
    }
}

impl fmt::Debug for OidcClientSecretDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcClientSecretDigest")
            .finish_non_exhaustive()
    }
}

impl fmt::Display for OidcClientSecretGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "operating system could not generate an OIDC client secret"
        )
    }
}

impl Error for OidcClientSecretGenerationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl fmt::Display for OidcClientSecretParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OIDC client secret has an invalid representation")
    }
}

impl Error for OidcClientSecretParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEncoding(error) => Some(error),
            Self::InvalidLength(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_secret_round_trips_and_verifies() {
        let secret = OidcClientSecret::generate().unwrap();
        let encoded = secret.encode();
        let parsed = OidcClientSecret::parse(&encoded).unwrap();

        assert!(secret.digest().verifies(&parsed));
        assert_eq!(encoded.len(), 43);
    }

    #[test]
    fn debug_output_redacts_secret_and_digest() {
        let secret = OidcClientSecret::generate().unwrap();
        let encoded = secret.encode();

        assert!(!format!("{secret:?}").contains(encoded.as_str()));
        assert!(!format!("{:?}", secret.digest()).contains(encoded.as_str()));
    }
}
