#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UserRole {
    PreAuthentication,
    User,
    Manager,
    Administrator,
    Owner,
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreAuthentication => "pre_authentication",
            Self::User => "user",
            Self::Manager => "manager",
            Self::Administrator => "administrator",
            Self::Owner => "owner",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pre_authentication" => Some(Self::PreAuthentication),
            "user" => Some(Self::User),
            "manager" => Some(Self::Manager),
            "administrator" => Some(Self::Administrator),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    pub fn is_administrator(self) -> bool {
        matches!(self, Self::Administrator | Self::Owner)
    }
}
