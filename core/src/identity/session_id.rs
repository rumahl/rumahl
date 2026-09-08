use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

#[derive(Debug)]
pub enum SessionIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, SessionIdError> {
        let uuid =
            Uuid::try_parse(value).map_err(SessionIdError::InvalidFormat)?;

        let version = uuid.get_version_num();

        if version != 7 {
            return Err(SessionIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for SessionIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => {
                write!(f, "invalid session ID: {error}")
            }

            Self::WrongVersion(version) => {
                write!(
                    f,
                    "session ID must be UUID version 7, found version {version}"
                )
            }
        }
    }
}

impl Error for SessionIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidFormat(error) => Some(error),
            Self::WrongVersion(_) => None,
        }
    }
}

impl TryFrom<&str> for SessionId {
    type Error = SessionIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for SessionId {
    type Error = SessionIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_uuid_v7() {
        let id = SessionId::new();

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(SessionId::new(), SessionId::new());
    }

    #[test]
    fn round_trip_preserves_session_id() {
        let original = SessionId::new();

        let parsed =
            SessionId::parse(&original.to_string()).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn rejects_invalid_uuid() {
        assert!(matches!(
            SessionId::parse("not-a-session"),
            Err(SessionIdError::InvalidFormat(_))
        ));
    }

    #[test]
    fn rejects_non_v7_uuid() {
        assert!(matches!(
            SessionId::parse(
                "550e8400-e29b-41d4-a716-446655440000"
            ),
            Err(SessionIdError::WrongVersion(4))
        ));
    }
}