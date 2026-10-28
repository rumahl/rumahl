use std::error::Error;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const SESSION_TOKEN_LENGTH: usize = 32;

pub struct SessionToken(Zeroizing<[u8; SESSION_TOKEN_LENGTH]>);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionTokenDigest([u8; 32]);

#[derive(Debug)]
pub struct SessionTokenGenerationError(getrandom::Error);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTokenParseError {
    InvalidEncoding(base64::DecodeError),
    InvalidLength(usize),
}

impl SessionToken {
    pub fn generate() -> Result<Self, SessionTokenGenerationError> {
        let mut bytes = Zeroizing::new([0_u8; SESSION_TOKEN_LENGTH]);

        getrandom::fill(&mut bytes[..]).map_err(SessionTokenGenerationError)?;

        Ok(Self(bytes))
    }

    pub fn parse(encoded: &str) -> Result<Self, SessionTokenParseError> {
        let decoded = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(SessionTokenParseError::InvalidEncoding)?,
        );

        if decoded.len() != SESSION_TOKEN_LENGTH {
            return Err(SessionTokenParseError::InvalidLength(decoded.len()));
        }

        let mut bytes = [0_u8; SESSION_TOKEN_LENGTH];
        bytes.copy_from_slice(&decoded);

        Ok(Self(Zeroizing::new(bytes)))
    }

    pub fn encode(&self) -> Zeroizing<String> {
        Zeroizing::new(URL_SAFE_NO_PAD.encode(&self.0[..]))
    }

    pub fn digest(&self) -> SessionTokenDigest {
        SessionTokenDigest(Sha256::digest(&self.0[..]).into())
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionToken").finish_non_exhaustive()
    }
}

impl SessionTokenDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SessionTokenDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionTokenDigest").finish_non_exhaustive()
    }
}

impl fmt::Display for SessionTokenGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "operating system could not generate a session credential"
        )
    }
}

impl Error for SessionTokenGenerationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl fmt::Display for SessionTokenParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session credential has an invalid representation")
    }
}

impl Error for SessionTokenParseError {
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
    fn generated_tokens_are_unique_and_url_safe() {
        let first = SessionToken::generate().unwrap();
        let second = SessionToken::generate().unwrap();
        let encoded = first.encode();

        assert_ne!(first.digest(), second.digest());
        assert_eq!(encoded.len(), 43);
        assert!(
            encoded.chars().all(
                |character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            )
        );
    }

    #[test]
    fn encoded_token_round_trips() {
        let token = SessionToken::generate().unwrap();
        let encoded = token.encode();
        let parsed = SessionToken::parse(&encoded).unwrap();

        assert_eq!(parsed.digest(), token.digest());
    }

    #[test]
    fn debug_output_never_contains_token() {
        let token = SessionToken::generate().unwrap();
        let encoded = token.encode();
        let debug = format!("{token:?}");

        assert_eq!(debug, "SessionToken { .. }");
        assert!(!debug.contains(encoded.as_str()));
    }

    #[test]
    fn rejects_wrong_token_length() {
        assert_eq!(
            SessionToken::parse("c2hvcnQ").unwrap_err(),
            SessionTokenParseError::InvalidLength(5)
        );
    }
}
