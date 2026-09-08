use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId(Uuid);

#[derive(Debug)]
pub enum CorrelationIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl CorrelationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, CorrelationIdError> {
        let uuid =
            Uuid::try_parse(value).map_err(CorrelationIdError::InvalidFormat)?;

        let version = uuid.get_version_num();

        if version != 7 {
            return Err(CorrelationIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for CorrelationIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => {
                write!(f, "invalid correlation ID: {error}")
            }

            Self::WrongVersion(version) => {
                write!(
                    f,
                    "correlation ID must be UUID version 7, found version {version}"
                )
            }
        }
    }
}

impl Error for CorrelationIdError {
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
    fn generates_uuid_v7() {
        let id = CorrelationId::new();

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(
            CorrelationId::new(),
            CorrelationId::new()
        );
    }

    #[test]
    fn round_trip_preserves_correlation_id() {
        let original = CorrelationId::new();

        let parsed =
            CorrelationId::parse(&original.to_string()).unwrap();

        assert_eq!(original, parsed);
    }
}