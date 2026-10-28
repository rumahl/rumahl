use std::error::Error;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

const CLIENT_ID_RANDOM_LENGTH: usize = 24;
const CLIENT_ID_PREFIX: &str = "rmh_";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OidcClientId(String);

#[derive(Debug)]
pub struct OidcClientIdGenerationError(getrandom::Error);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcClientIdParseError {
    InvalidPrefix,
    InvalidEncoding(base64::DecodeError),
    InvalidLength(usize),
    NonCanonical,
}

impl OidcClientId {
    pub fn generate() -> Result<Self, OidcClientIdGenerationError> {
        let mut bytes = [0_u8; CLIENT_ID_RANDOM_LENGTH];
        getrandom::fill(&mut bytes).map_err(OidcClientIdGenerationError)?;

        Ok(Self(format!(
            "{CLIENT_ID_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(bytes)
        )))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, OidcClientIdParseError> {
        let value = value.into();
        let encoded = value
            .strip_prefix(CLIENT_ID_PREFIX)
            .ok_or(OidcClientIdParseError::InvalidPrefix)?;
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(OidcClientIdParseError::InvalidEncoding)?;

        if decoded.len() != CLIENT_ID_RANDOM_LENGTH {
            return Err(OidcClientIdParseError::InvalidLength(decoded.len()));
        }

        if URL_SAFE_NO_PAD.encode(decoded) != encoded {
            return Err(OidcClientIdParseError::NonCanonical);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OidcClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for OidcClientIdGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "operating system could not generate an OIDC client ID")
    }
}

impl Error for OidcClientIdGenerationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl fmt::Display for OidcClientIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OIDC client ID has an invalid representation")
    }
}

impl Error for OidcClientIdParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEncoding(error) => Some(error),
            Self::InvalidPrefix | Self::InvalidLength(_) | Self::NonCanonical => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_client_ids_are_unique_and_round_trip() {
        let first = OidcClientId::generate().unwrap();
        let second = OidcClientId::generate().unwrap();

        assert_ne!(first, second);
        assert_eq!(OidcClientId::parse(first.to_string()).unwrap(), first);
        assert!(first.as_str().starts_with(CLIENT_ID_PREFIX));
    }
}
