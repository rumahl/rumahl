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
    /// Resolves a home-relative path to the entries from the root to the
    /// target. An empty path resolves to the root and yields an empty trail.
    /// Every entry before the last must be a directory.
    fn path_trail(&self, user: UserId, path: &str) -> Result<Vec<PersonalFile>, FileError>;
    /// One entry by id, without reading its content. Used by quota checks and
    /// anything that needs a file's size without its bytes.
    fn entry(&self, user: UserId, id: &str) -> Result<PersonalFile, FileError>;
    /// The bytes the account's documents occupy, files only.
    fn usage(&self, user: UserId) -> Result<u64, FileError>;
    /// The stable id and kind of a home-relative path. An empty path is the
    /// root. This is what lets paths be addressed without exposing row ids.
    fn resolve_path(&self, user: UserId, path: &str) -> Result<(String, bool), FileError> {
        let trail = self.path_trail(user, path)?;
        Ok(match trail.last() {
            Some(entry) => (entry.id.clone(), entry.directory),
            None => ("root".to_owned(), true),
        })
    }
}

/// Splits a home-relative personal path into validated segments.
///
/// This is the one place a path becomes a sequence of names: no leading
/// slash, no `..`, no backslash, no control characters, at most 255 bytes per
/// name and 4096 bytes overall. Empty segments and `.` are ignored, matching
/// the stores' name rules.
pub fn personal_path_segments(path: &str) -> Result<Vec<&str>, FileError> {
    if path.len() > 4096 || path.starts_with('/') || path.contains(['\\', '\0']) {
        return Err(FileError::Invalid);
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.len() > 255 || segment.chars().any(char::is_control) {
            return Err(FileError::Invalid);
        }
        segments.push(segment);
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_paths_never_escape_or_use_host_names() {
        assert_eq!(
            personal_path_segments("Documents/a.txt").unwrap(),
            ["Documents", "a.txt"]
        );
        assert_eq!(
            personal_path_segments("/Documents").unwrap_err(),
            FileError::Invalid
        );
        assert_eq!(
            personal_path_segments("Documents/../etc").unwrap_err(),
            FileError::Invalid
        );
        assert_eq!(
            personal_path_segments("Documents\\a").unwrap_err(),
            FileError::Invalid
        );
        assert_eq!(
            personal_path_segments("a\0b").unwrap_err(),
            FileError::Invalid
        );
        assert_eq!(
            personal_path_segments(&"x".repeat(256)).unwrap_err(),
            FileError::Invalid
        );
        assert_eq!(personal_path_segments("a//./b/").unwrap(), ["a", "b"]);
    }
}
