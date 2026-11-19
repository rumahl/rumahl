//! Personal documents stored as real files in each account's home directory.
//!
//! The home (`/home/<username>`) is the source of truth for user files: the
//! explorer, editors and other tools see the same documents. This
//! implementation keeps the [`PersonalFiles`] contract:
//!
//! * File ids are opaque, stable and rename-safe. A new file derives its id
//!   from the owning account and its path; the id index remembers it, so a
//!   rename through rumahl never invalidates a files grant. Files created
//!   outside rumahl get their derived id when they are first listed.
//! * Paths are resolved inside the account's own home, never followed through
//!   symlinks and never above it.
//! * Name rules match the SQLite store: no separators, no control characters,
//!   no `.`/`..`, at most 255 bytes.
//! * With a [`FieldKeyProvider`] file content stays sealed inside the home
//!   (the deployment's data volume is not assumed to be encrypted); without
//!   one the files are plain, exactly as other tools expect.
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rumahl_core::{AccountStateRepository, FileError as E, PersonalFile, PersonalFiles, UserId};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::{FieldEnvelope, FieldKeyProvider, open_field, seal_field};

/// Sealed file marker, so sealed and plain files can coexist in one home.
const SEALED_MAGIC: &[u8; 4] = b"RHF1";
/// Mirrors the personal-files body limit of the HTTP API.
const MAX_CONTENT_BYTES: usize = 16 * 1024 * 1024;
/// Bounded directory listing, like the store.
const MAX_ENTRIES: usize = 1001;

fn content_context(owner: UserId, id: &str) -> String {
    format!("home-file:{owner}:{id}")
}

fn name_valid(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 255
        && ![".", ".."].contains(&name)
        && !name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
}

fn db(_: rusqlite::Error) -> E {
    E::Unavailable
}

fn io_error(error: io::Error) -> E {
    if error.kind() == io::ErrorKind::NotFound {
        E::Missing
    } else {
        E::Unavailable
    }
}

/// One account's home directory with a stable id index and optional sealing.
pub struct HomePersonalFiles {
    home_root: PathBuf,
    accounts: PathBuf,
    index: PathBuf,
    key_provider: Option<Arc<dyn FieldKeyProvider>>,
}

impl HomePersonalFiles {
    /// Plain files in the home (deployments with an encrypted data volume).
    pub fn open(
        home_root: impl AsRef<Path>,
        accounts: impl AsRef<Path>,
        index: impl AsRef<Path>,
    ) -> Result<Self, rusqlite::Error> {
        Self::open_with(home_root, accounts, index, None)
    }

    /// Sealed files in the home: the path is real, the content is not readable
    /// without the account's key.
    pub fn open_encrypted(
        home_root: impl AsRef<Path>,
        accounts: impl AsRef<Path>,
        index: impl AsRef<Path>,
        key_provider: Arc<dyn FieldKeyProvider>,
    ) -> Result<Self, rusqlite::Error> {
        Self::open_with(home_root, accounts, index, Some(key_provider))
    }

    fn open_with(
        home_root: impl AsRef<Path>,
        accounts: impl AsRef<Path>,
        index: impl AsRef<Path>,
        key_provider: Option<Arc<dyn FieldKeyProvider>>,
    ) -> Result<Self, rusqlite::Error> {
        let this = Self {
            home_root: home_root.as_ref().into(),
            accounts: accounts.as_ref().into(),
            index: index.as_ref().into(),
            key_provider,
        };
        let connection = this.connect()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS home_file_index (owner TEXT NOT NULL, id TEXT NOT NULL, path TEXT NOT NULL, PRIMARY KEY(owner,id), UNIQUE(owner,path));",
        )?;
        Ok(this)
    }

    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(&self.index)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        Ok(connection)
    }

    /// The account's home, canonicalized and verified to be a directory.
    fn home(&self, owner: UserId) -> Result<PathBuf, E> {
        let state = crate::SqliteAccountStateRepository::open(&self.accounts)
            .and_then(|repository| repository.load())
            .map_err(|_| E::Unavailable)?;
        let username = state
            .accounts()
            .get(&owner)
            .map(|account| account.username().as_str().to_owned())
            .ok_or(E::Missing)?;
        let root = self.home_root.canonicalize().map_err(|_| E::Unavailable)?;
        let canonical = root.join(&username).canonicalize().map_err(io_error)?;
        canonical
            .starts_with(&root)
            .then_some(canonical)
            .ok_or(E::Invalid)
    }

    /// Normalizes a home-relative path: segments only, no traversal.
    fn normalize(path: &str) -> Result<Vec<String>, E> {
        let mut segments = Vec::new();
        for segment in path.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." || segment.contains('\\') || segment.contains('\0') {
                return Err(E::Invalid);
            }
            segments.push(segment.to_owned());
        }
        Ok(segments)
    }

    fn derived_id(owner: UserId, path: &str) -> String {
        let digest = Sha256::digest(format!("{owner}\0{path}").as_bytes());
        digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The id for a path: the remembered one, or the derived id when free.
    fn id_for_path(connection: &Connection, owner: UserId, path: &str) -> Result<String, E> {
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM home_file_index WHERE owner=?1 AND path=?2",
                params![owner.to_string(), path],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = Self::derived_id(owner, path);
        let taken: Option<String> = connection
            .query_row(
                "SELECT path FROM home_file_index WHERE owner=?1 AND id=?2",
                params![owner.to_string(), id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?;
        // A derived id that already names another path is a collision, never a
        // reason to merge two files onto one id.
        if taken.as_deref().is_some_and(|other| other != path) {
            return Err(E::Conflict);
        }
        connection
            .execute(
                "INSERT INTO home_file_index (owner, id, path) VALUES (?1, ?2, ?3) ON CONFLICT(owner, path) DO NOTHING",
                params![owner.to_string(), id, path],
            )
            .map_err(db)?;
        Ok(id)
    }

    fn path_for_id(connection: &Connection, owner: UserId, id: &str) -> Result<String, E> {
        connection
            .query_row(
                "SELECT path FROM home_file_index WHERE owner=?1 AND id=?2",
                params![owner.to_string(), id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?
            .ok_or(E::Missing)
    }

    /// Resolves an id to its filesystem path, ensuring it stays inside the home.
    fn resolve(&self, owner: UserId, id: &str) -> Result<(PathBuf, String), E> {
        let connection = self.connect().map_err(db)?;
        let relative = Self::path_for_id(&connection, owner, id)?;
        let home = self.home(owner)?;
        let mut target = home.clone();
        for segment in Self::normalize(&relative)? {
            target.push(segment);
        }
        let canonical = target.canonicalize().map_err(io_error)?;
        if !canonical.starts_with(&home) {
            return Err(E::Invalid);
        }
        Ok((canonical, relative))
    }

    /// Whether `id` names an entry in the account's home. Used by storage
    /// migrations to decide whether a grant still points at a servable file.
    pub fn contains(&self, owner: UserId, id: &str) -> bool {
        self.resolve(owner, id).is_ok()
    }

    /// Resolves a listing parent ("root" or a directory id) to its path.
    fn resolve_directory(&self, owner: UserId, parent: &str) -> Result<(PathBuf, String), E> {
        if parent == "root" {
            return Ok((self.home(owner)?, String::new()));
        }
        let (directory, relative) = self.resolve(owner, parent)?;
        let meta = std::fs::symlink_metadata(&directory).map_err(io_error)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(E::Invalid);
        }
        Ok((directory, relative))
    }

    fn join(relative: &str, name: &str) -> String {
        if relative.is_empty() {
            name.to_owned()
        } else {
            format!("{relative}/{name}")
        }
    }

    fn seal(&self, owner: UserId, id: &str, data: &[u8]) -> Result<Vec<u8>, E> {
        let Some(provider) = self.key_provider.as_ref() else {
            return Ok(data.to_vec());
        };
        let envelope = seal_field(
            provider.as_ref(),
            content_context(owner, id).as_bytes(),
            data,
        )
        .map_err(|_| E::Unavailable)?;
        let mut sealed = Vec::with_capacity(SEALED_MAGIC.len() + data.len() + 64);
        sealed.extend_from_slice(SEALED_MAGIC);
        sealed.extend_from_slice(&envelope.encode());
        Ok(sealed)
    }

    fn open_content(&self, owner: UserId, id: &str, stored: Vec<u8>) -> Result<Vec<u8>, E> {
        let Some(rest) = stored.strip_prefix(SEALED_MAGIC) else {
            // A plain file: created by another tool or before sealing was on.
            return Ok(stored);
        };
        let provider = self.key_provider.as_ref().ok_or(E::Unavailable)?;
        let envelope = FieldEnvelope::decode(rest).map_err(|_| E::Unavailable)?;
        let plaintext = open_field(
            provider.as_ref(),
            content_context(owner, id).as_bytes(),
            &envelope,
        )
        .map_err(|_| E::Unavailable)?;
        Ok(plaintext.to_vec())
    }

    fn entry(
        &self,
        connection: &Connection,
        home: &Path,
        owner: UserId,
        parent: &str,
        relative: &str,
        name: &str,
    ) -> Result<Option<PersonalFile>, E> {
        let child_relative = Self::join(relative, name);
        let target = home.join(&child_relative);
        let meta = match std::fs::symlink_metadata(&target) {
            Ok(meta) => meta,
            Err(_) => return Ok(None),
        };
        if meta.file_type().is_symlink() {
            return Ok(None);
        }
        let directory = meta.is_dir();
        let id = Self::id_for_path(connection, owner, &child_relative)?;
        let children = if directory {
            std::fs::read_dir(&target)
                .map(|entries| entries.take(1024).count() as u32)
                .unwrap_or(0)
        } else {
            0
        };
        Ok(Some(PersonalFile {
            id,
            parent: parent.to_owned(),
            name: name.to_owned(),
            directory,
            size: if directory { 0 } else { meta.len() },
            children,
        }))
    }
}

impl PersonalFiles for HomePersonalFiles {
    fn list(&self, owner: UserId, parent: &str) -> Result<Vec<PersonalFile>, E> {
        let (directory, relative) = self.resolve_directory(owner, parent)?;
        let home = self.home(owner)?;
        let connection = self.connect().map_err(db)?;
        let mut items = Vec::new();
        for entry in std::fs::read_dir(&directory).map_err(io_error)? {
            if items.len() >= MAX_ENTRIES {
                break;
            }
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(item) = self.entry(&connection, &home, owner, parent, &relative, &name)? {
                items.push(item);
            }
        }
        items.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(items)
    }

    fn create(
        &self,
        owner: UserId,
        parent: &str,
        name: &str,
        data: Option<Vec<u8>>,
    ) -> Result<(), E> {
        let (directory, relative) = self.resolve_directory(owner, parent)?;
        if !name_valid(name) {
            return Err(E::Invalid);
        }
        let target = directory.join(name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(E::Conflict);
        }
        let path = Self::join(&relative, name);
        match data {
            Some(data) => {
                if data.len() > MAX_CONTENT_BYTES {
                    return Err(E::Limit);
                }
                // The id binds the sealed content, so it is assigned first and
                // rolled back if the file cannot be written.
                let connection = self.connect().map_err(db)?;
                let id = Self::id_for_path(&connection, owner, &path)?;
                let sealed = self.seal(owner, &id, &data)?;
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true).mode(0o600);
                let written =
                    options
                        .open(&target)
                        .map_err(|_| E::Conflict)
                        .and_then(|mut file| {
                            use std::io::Write;
                            file.write_all(&sealed).map_err(|_| E::Unavailable)
                        });
                if let Err(error) = written {
                    let _ = std::fs::remove_file(&target);
                    let _ = connection.execute(
                        "DELETE FROM home_file_index WHERE owner=?1 AND id=?2",
                        params![owner.to_string(), id],
                    );
                    return Err(error);
                }
                Ok(())
            }
            None => {
                let mut builder = std::fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(&target).map_err(|_| E::Conflict)?;
                let connection = self.connect().map_err(db)?;
                Self::id_for_path(&connection, owner, &path)?;
                Ok(())
            }
        }
    }

    fn read(&self, owner: UserId, id: &str) -> Result<(String, Vec<u8>), E> {
        let (path, _) = self.resolve(owner, id)?;
        let meta = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(E::Missing);
        }
        // Sealed content is the plaintext plus a small envelope.
        if meta.len() > (MAX_CONTENT_BYTES as u64) + 1024 {
            return Err(E::Limit);
        }
        let stored = std::fs::read(&path).map_err(io_error)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((name, self.open_content(owner, id, stored)?))
    }

    fn write(&self, owner: UserId, id: &str, data: Vec<u8>) -> Result<(), E> {
        if data.len() > MAX_CONTENT_BYTES {
            return Err(E::Limit);
        }
        let (path, _) = self.resolve(owner, id)?;
        let meta = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(E::Missing);
        }
        let sealed = self.seal(owner, id, &data)?;
        let directory = path.parent().ok_or(E::Invalid)?;
        let temporary = directory.join(format!(
            ".{}.tmp-{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            UserId::new()
        ));
        {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| E::Unavailable)?;
            file.write_all(&sealed).map_err(|_| E::Unavailable)?;
            file.sync_all().map_err(|_| E::Unavailable)?;
        }
        std::fs::rename(&temporary, &path).map_err(|_| E::Unavailable)
    }

    fn relocate(&self, owner: UserId, id: &str, parent: &str, name: &str) -> Result<(), E> {
        if !name_valid(name) {
            return Err(E::Invalid);
        }
        let (source, source_relative) = self.resolve(owner, id)?;
        let (destination_directory, destination_relative) =
            self.resolve_directory(owner, parent)?;
        let destination = destination_directory.join(name);
        // Moving a folder into its own subtree would detach it from the home.
        if destination.starts_with(&source) {
            return Err(E::Invalid);
        }
        if std::fs::symlink_metadata(&destination).is_ok() {
            return Err(E::Conflict);
        }
        let destination_relative = Self::join(&destination_relative, name);
        if !destination_directory.starts_with(self.home(owner)?) {
            return Err(E::Invalid);
        }
        std::fs::rename(&source, &destination).map_err(|_| E::Unavailable)?;
        let connection = self.connect().map_err(db)?;
        // The id must survive the rename: grants reference it.
        connection
            .execute(
                "UPDATE home_file_index SET path=?3 WHERE owner=?1 AND id=?2 AND path=?4",
                params![owner.to_string(), id, destination_relative, source_relative],
            )
            .map_err(db)?;
        // Descendant rows move with the folder.
        let prefix = format!("{source_relative}/");
        let moved: Vec<(String, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT id, path FROM home_file_index WHERE owner=?1 AND path LIKE ?2 || '%'",
                )
                .map_err(db)?;
            let rows = statement
                .query_map(params![owner.to_string(), prefix], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(db)?;
            rows.filter_map(Result::ok).collect()
        };
        for (child_id, child_path) in moved {
            let suffix = &child_path[prefix.len()..];
            connection
                .execute(
                    "UPDATE home_file_index SET path=?3 WHERE owner=?1 AND id=?2",
                    params![
                        owner.to_string(),
                        child_id,
                        format!("{destination_relative}/{suffix}")
                    ],
                )
                .map_err(db)?;
        }
        Ok(())
    }

    fn entry(&self, owner: UserId, id: &str) -> Result<PersonalFile, E> {
        let (path, relative) = self.resolve(owner, id)?;
        let meta = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if meta.file_type().is_symlink() {
            return Err(E::Invalid);
        }
        let directory = meta.is_dir();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent_id = if relative.is_empty() {
            "root".to_owned()
        } else {
            match relative.rsplit_once('/') {
                Some((parent, _)) => self
                    .connect()
                    .map_err(db)
                    .and_then(|c| Self::id_for_path(&c, owner, parent))
                    .unwrap_or_default(),
                None => "root".to_owned(),
            }
        };
        Ok(PersonalFile {
            id: id.to_owned(),
            parent: parent_id,
            name,
            directory,
            size: if directory { 0 } else { meta.len() },
            children: 0,
        })
    }

    fn usage(&self, owner: UserId) -> Result<u64, E> {
        let home = self.home(owner)?;
        let mut used = 0_u64;
        let mut visited = 0_usize;
        // Bounded like every other walk: a home that grew past these bounds
        // reports its whole budget as used instead of hanging a request.
        const MAX_USAGE_ENTRIES: usize = 100_000;
        const MAX_USAGE_DEPTH: usize = 64;
        fn walk_bounded(
            dir: &Path,
            depth: usize,
            used: &mut u64,
            visited: &mut usize,
        ) -> std::io::Result<()> {
            if depth > MAX_USAGE_DEPTH {
                return Ok(());
            }
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let meta = std::fs::symlink_metadata(entry.path())?;
                if meta.file_type().is_symlink() {
                    continue;
                }
                *visited += 1;
                if *visited >= MAX_USAGE_ENTRIES {
                    return Ok(());
                }
                if meta.is_dir() {
                    walk_bounded(&entry.path(), depth + 1, used, visited)?;
                } else if meta.is_file() {
                    *used = used.saturating_add(meta.len());
                }
            }
            Ok(())
        }
        walk_bounded(&home, 0, &mut used, &mut visited).map_err(|_| E::Unavailable)?;
        Ok(used)
    }

    fn path_trail(&self, owner: UserId, path: &str) -> Result<Vec<PersonalFile>, E> {
        let segments = rumahl_core::personal_path_segments(path)?;
        let home = self.home(owner)?;
        let connection = self.connect().map_err(db)?;
        let mut target = home.clone();
        let mut relative = String::new();
        let mut parent = "root".to_owned();
        let mut trail: Vec<PersonalFile> = Vec::with_capacity(segments.len());
        for (position, segment) in segments.iter().enumerate() {
            target.push(segment);
            let meta = std::fs::symlink_metadata(&target).map_err(io_error)?;
            if meta.file_type().is_symlink() {
                return Err(E::Invalid);
            }
            let directory = meta.is_dir();
            if position + 1 < segments.len() && !directory {
                return Err(E::Invalid);
            }
            relative = Self::join(&relative, segment);
            let id = Self::id_for_path(&connection, owner, &relative)?;
            trail.push(PersonalFile {
                id: id.clone(),
                parent: parent.clone(),
                name: (*segment).to_owned(),
                directory,
                size: if directory { 0 } else { meta.len() },
                children: 0,
            });
            parent = id;
        }
        Ok(trail)
    }

    fn delete(&self, owner: UserId, id: &str) -> Result<(), E> {
        let (path, relative) = self.resolve(owner, id)?;
        let meta = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if meta.file_type().is_symlink() {
            return Err(E::Invalid);
        }
        if meta.is_dir() {
            std::fs::remove_dir(&path).map_err(|_| E::Conflict)?;
        } else {
            std::fs::remove_file(&path).map_err(|_| E::Unavailable)?;
        }
        let connection = self.connect().map_err(db)?;
        connection
            .execute(
                "DELETE FROM home_file_index WHERE owner=?1 AND id=?2",
                params![owner.to_string(), id],
            )
            .map_err(db)?;
        // Drop stale index rows for descendants (folders delete empty only, so
        // this guards against rows left by files removed outside rumahl).
        let prefix = format!("{relative}/");
        connection
            .execute(
                "DELETE FROM home_file_index WHERE owner=?1 AND path LIKE ?2 || '%'",
                params![owner.to_string(), prefix],
            )
            .map_err(db)?;
        Ok(())
    }
}

impl std::fmt::Debug for HomePersonalFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomePersonalFiles")
            .field("home_root", &self.home_root)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}
