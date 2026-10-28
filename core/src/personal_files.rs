//! Private user documents, distinct from host paths and app filesystem grants.
use crate::UserId;
#[derive(Debug, Clone)]
pub struct PersonalFile {
    pub id: String,
    pub parent: String,
    pub name: String,
    pub directory: bool,
    pub size: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    Missing,
    Invalid,
    Conflict,
    Limit,
    Unavailable,
}
pub trait PersonalFiles: Send + Sync {
    fn list(&self, user: UserId, parent: &str) -> Result<Vec<PersonalFile>, FileError>;
    fn create(
        &self,
        user: UserId,
        parent: &str,
        name: &str,
        data: Option<Vec<u8>>,
    ) -> Result<(), FileError>;
    fn read(&self, user: UserId, id: &str) -> Result<(String, Vec<u8>), FileError>;
    fn relocate(&self, user: UserId, id: &str, parent: &str, name: &str) -> Result<(), FileError>;
    /// Directories must be empty; deletion never follows paths or recursively deletes.
    fn delete(&self, user: UserId, id: &str) -> Result<(), FileError>;
}
