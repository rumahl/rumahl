use std::error::Error;
use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(Uuid);

#[derive(Debug)]
pub enum UserIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, UserIdError> {
        let uuid = Uuid::try_parse(value).map_err(UserIdError::InvalidFormat)?;

        let version = uuid.get_version_num();

        if version != 7 {
            return Err(UserIdError::WrongVersion(version));
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for UserIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => {
                write!(f, "invalid user ID: {error}")
            }

            Self::WrongVersion(version) => {
                write!(f, "user ID must be UUID version 7, found version {version}")
            }
        }
    }
}

impl Error for UserIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidFormat(error) => Some(error),
            Self::WrongVersion(_) => None,
        }
    }
}

impl TryFrom<&str> for UserId {
    type Error = UserIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for UserId {
    type Error = UserIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_uuid_v7_user_id() {
        let id = UserId::new();

        assert_eq!(id.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn generated_user_ids_are_unique() {
        let first = UserId::new();
        let second = UserId::new();

        assert_ne!(first, second);
    }

    #[test]
    fn parses_existing_user_id() {
        let original = UserId::new();

        let parsed = UserId::parse(&original.to_string()).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn rejects_invalid_user_id() {
        assert!(matches!(
            UserId::parse("kai"),
            Err(UserIdError::InvalidFormat(_))
        ));
    }

    #[test]
    fn rejects_non_v7_uuid() {
        let result = UserId::parse("550e8400-e29b-41d4-a716-446655440000");

        assert!(matches!(result, Err(UserIdError::WrongVersion(4))));
    }
}
