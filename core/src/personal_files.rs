//! Private user documents, distinct from host paths and app filesystem grants.
use crate::UserId;
#[derive(Debug, Clone)]
pub struct PersonalFile {
    pub id: String,
    pub parent: String,
    pub name: String,
    pub directory: bool,
    pub size: u64,
    /// Direct children for folders; zero for files.
    pub children: u32,
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
    /// Paginated, optionally name-filtered listing. Implementations backed by a
    /// database should override this with a real LIMIT/OFFSET query; the
    /// default keeps in-memory test doubles working.
    fn list_page(
        &self,
        user: UserId,
        parent: &str,
        offset: usize,
        limit: usize,
        query: Option<&str>,
    ) -> Result<(Vec<PersonalFile>, u64), FileError> {
        let items = self.list(user, parent)?;
        let needle = query
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
        let filtered: Vec<PersonalFile> = match needle {
            Some(needle) => items
                .into_iter()
                .filter(|item| item.name.to_lowercase().contains(&needle))
                .collect(),
            None => items,
        };
        let total = filtered.len() as u64;
        Ok((
            filtered.into_iter().skip(offset).take(limit).collect(),
            total,
        ))
    }
    fn create(
        &self,
        user: UserId,
        parent: &str,
        name: &str,
        data: Option<Vec<u8>>,
    ) -> Result<(), FileError>;
    fn read(&self, user: UserId, id: &str) -> Result<(String, Vec<u8>), FileError>;
    /// Replaces a file's content in place. Directories are never writable.
    fn write(&self, user: UserId, id: &str, data: Vec<u8>) -> Result<(), FileError>;
    fn relocate(&self, user: UserId, id: &str, parent: &str, name: &str) -> Result<(), FileError>;
    /// Directories must be empty; deletion never follows paths or recursively deletes.
    fn delete(&self, user: UserId, id: &str) -> Result<(), FileError>;
}
