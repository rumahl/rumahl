use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppOperationId(Uuid);

#[derive(Debug)]
pub enum AppOperationIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl AppOperationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, AppOperationIdError> {
        let uuid = Uuid::try_parse(value).map_err(AppOperationIdError::InvalidFormat)?;
        let version = uuid.get_version_num();

        if version != 7 {
            return Err(AppOperationIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for AppOperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AppOperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for AppOperationIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => write!(f, "invalid app operation ID: {error}"),
            Self::WrongVersion(version) => write!(
                f,
                "app operation ID must be UUID version 7, found version {version}"
            ),
        }
    }
}

impl Error for AppOperationIdError {
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
    fn round_trip_preserves_operation_id() {
        let id = AppOperationId::new();

        assert_eq!(AppOperationId::parse(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn rejects_non_v7_uuid() {
        assert!(matches!(
            AppOperationId::parse("550e8400-e29b-41d4-a716-446655440000"),
            Err(AppOperationIdError::WrongVersion(_))
        ));
    }
}
