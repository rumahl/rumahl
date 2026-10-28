use std::error::Error;
use std::fmt;

use zeroize::{Zeroize, Zeroizing};

pub struct SecretValue(Zeroizing<Vec<u8>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretValueError {
    Empty,
    TooLarge,
}

impl SecretValue {
    pub const MAX_LENGTH: usize = 64 * 1024;

    pub fn new(mut value: Vec<u8>) -> Result<Self, SecretValueError> {
        if value.is_empty() {
            value.zeroize();
            return Err(SecretValueError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            value.zeroize();
            return Err(SecretValueError::TooLarge);
        }

        Ok(Self(Zeroizing::new(value)))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretValue")
            .field("length", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for SecretValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "secret value cannot be empty"),
            Self::TooLarge => write!(
                f,
                "secret value cannot exceed {} bytes",
                SecretValue::MAX_LENGTH
            ),
        }
    }
}

impl Error for SecretValueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_contains_secret() {
        let value = SecretValue::new(b"do-not-print-this".to_vec()).unwrap();

        assert!(!format!("{value:?}").contains("do-not-print-this"));
    }
}
