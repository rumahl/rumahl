#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UserRole {
    PreAuthentication,
    User,
    Manager,
    Administrator,
    Owner,
}
