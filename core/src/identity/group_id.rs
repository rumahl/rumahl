use std::error::Error;
use std::fmt;

use uuid::Uuid;

/// Stable identifier of an account group. Groups carry policy assignments the
/// same way users do, so a group must have the same identity hygiene as an
/// account: opaque, versioned, never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(Uuid);

#[derive(Debug)]
pub enum GroupIdError {
    InvalidFormat(uuid::Error),
    WrongVersion(usize),
}

impl GroupId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, GroupIdError> {
        let uuid = Uuid::try_parse(value).map_err(GroupIdError::InvalidFormat)?;
        let version = uuid.get_version_num();
        if version != 7 {
            return Err(GroupIdError::WrongVersion(version));
        }
        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for GroupId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for GroupIdError {}

impl fmt::Display for GroupIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(error) => write!(f, "invalid group id: {error}"),
            Self::WrongVersion(version) => write!(f, "unsupported group id version: v{version}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_foreign_uuids() {
        let id = GroupId::new();
        assert_eq!(GroupId::parse(&id.to_string()).unwrap(), id);
        // A v4 UUID string is not a group id: ids must stay tamper-evident.
        let v4 = "0199c0de-4000-4000-8000-000000000000";
        assert!(GroupId::parse(v4).is_err());
        assert!(GroupId::parse("not-a-uuid").is_err());
    }
}
