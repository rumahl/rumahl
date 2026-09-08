use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstallationId(Uuid);

#[derive(Debug)]
pub enum InstallationIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl InstallationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, InstallationIdError> {
        let uuid =
            Uuid::try_parse(value).map_err(InstallationIdError::InvalidFormat)?;

        let version = uuid.get_version_num();

        if version != 7 {
            return Err(InstallationIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for InstallationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InstallationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for InstallationIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => {
                write!(f, "invalid installation ID: {error}")
            }

            Self::WrongVersion(version) => {
                write!(
                    f,
                    "installation ID must be UUID version 7, found version {version}"
                )
            }
        }
    }
}

impl Error for InstallationIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidFormat(error) => Some(error),
            Self::WrongVersion(_) => None,
        }
    }
}

impl TryFrom<&str> for InstallationId {
    type Error = InstallationIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for InstallationId {
    type Error = InstallationIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_uuid_v7() {
        let id = InstallationId::new();

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn generated_ids_are_unique() {
        let first = InstallationId::new();
        let second = InstallationId::new();

        assert_ne!(first, second);
    }

    #[test]
    fn parses_valid_uuid_v7() {
        let generated = InstallationId::new();

        let parsed =
            InstallationId::parse(&generated.to_string()).unwrap();

        assert_eq!(generated, parsed);
    }

    #[test]
    fn rejects_invalid_uuid() {
        let result = InstallationId::parse("not-a-uuid");

        assert!(matches!(
            result,
            Err(InstallationIdError::InvalidFormat(_))
        ));
    }
}

#[test]
fn rejects_non_v7_uuid() {
    let result =
        InstallationId::parse("550e8400-e29b-41d4-a716-446655440000");

    assert!(matches!(
        result,
        Err(InstallationIdError::WrongVersion(_))
    ));
}