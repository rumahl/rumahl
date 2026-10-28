#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionScope {
    AppPrivate,
    UserOwn,
    UserSelected,
    Explicit,
    FamilyShared,
    System,
}
