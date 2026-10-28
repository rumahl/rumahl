use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrantId(Uuid);

#[derive(Debug)]
pub enum GrantIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl GrantId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, GrantIdError> {
        let uuid = Uuid::try_parse(value).map_err(GrantIdError::InvalidFormat)?;
        let version = uuid.get_version_num();

        if version != 7 {
            return Err(GrantIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for GrantId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GrantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for GrantIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => write!(f, "invalid grant ID: {error}"),
            Self::WrongVersion(version) => {
                write!(
                    f,
                    "grant ID must be UUID version 7, found version {version}"
                )
            }
        }
    }
}

impl Error for GrantIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidFormat(error) => Some(error),
            Self::WrongVersion(_) => None,
        }
    }
}

impl TryFrom<&str> for GrantId {
    type Error = GrantIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for GrantId {
    type Error = GrantIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_grant_id() {
        let generated = GrantId::new();

        assert_eq!(GrantId::parse(&generated.to_string()).unwrap(), generated);
    }

    #[test]
    fn rejects_invalid_uuid() {
        assert!(matches!(
            GrantId::parse("not-a-uuid"),
            Err(GrantIdError::InvalidFormat(_))
        ));
    }

    #[test]
    fn rejects_non_v7_uuid() {
        assert!(matches!(
            GrantId::parse("550e8400-e29b-41d4-a716-446655440000"),
            Err(GrantIdError::WrongVersion(_))
        ));
    }
}
