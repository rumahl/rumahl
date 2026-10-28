use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretId(Uuid);

#[derive(Debug)]
pub enum SecretIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl SecretId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, SecretIdError> {
        let uuid = Uuid::try_parse(value).map_err(SecretIdError::InvalidFormat)?;
        let version = uuid.get_version_num();

        if version != 7 {
            return Err(SecretIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }
}

impl Default for SecretId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for SecretIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => write!(f, "invalid secret ID: {error}"),
            Self::WrongVersion(version) => write!(
                f,
                "secret ID must be UUID version 7, found version {version}"
            ),
        }
    }
}

impl Error for SecretIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidFormat(error) => Some(error),
            Self::WrongVersion(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_id_round_trips() {
        let id = SecretId::new();

        assert_eq!(SecretId::parse(&id.to_string()).unwrap(), id);
    }
}
