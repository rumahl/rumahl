use rumahl_core::{FileError as E, PersonalFile, PersonalFiles, UserId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::schema::ensure_column;
use crate::{FieldEnvelope, FieldKeyProvider, open_field, seal_field};

/// Personal documents, stored in one SQLite database.
///
/// With a [`FieldKeyProvider`] the file content is sealed with AES-256-GCM and
/// bound to its owning row (see [`open_encrypted`](Self::open_encrypted)).
/// Without one (development) content is stored as plaintext; the `encrypted`
/// column records which rows are sealed so both shapes can coexist.
pub struct SqlitePersonalFiles {
    path: PathBuf,
    key_provider: Option<Arc<dyn FieldKeyProvider>>,
}

impl SqlitePersonalFiles {
    /// Opens the store without content encryption (development only).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        Self::open_with(path, None)
    }

    /// Opens the store and seals file content with the given key provider.
    pub fn open_encrypted(
        path: impl AsRef<Path>,
        key_provider: Arc<dyn FieldKeyProvider>,
    ) -> Result<Self, rusqlite::Error> {
        Self::open_with(path, Some(key_provider))
    }

    fn open_with(
        path: impl AsRef<Path>,
        key_provider: Option<Arc<dyn FieldKeyProvider>>,
    ) -> Result<Self, rusqlite::Error> {
        let this = Self {
            path: path.as_ref().into(),
            key_provider,
        };
        let connection = this.connect()?;
        connection.execute_batch("PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS personal_files (owner TEXT NOT NULL,id TEXT NOT NULL,parent TEXT NOT NULL,name TEXT NOT NULL,directory INTEGER NOT NULL,data BLOB,size INTEGER NOT NULL DEFAULT 0,encrypted INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(owner,id),UNIQUE(owner,parent,name)); CREATE INDEX IF NOT EXISTS personal_files_parent ON personal_files(owner,parent);")?;
        ensure_column(
            &connection,
            "personal_files",
            "size",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &connection,
            "personal_files",
            "encrypted",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        // Backfill the plaintext size for rows created before the column existed.
        connection.execute_batch("UPDATE personal_files SET size = COALESCE(length(data),0) WHERE size = 0 AND data IS NOT NULL AND directory = 0")?;
        Ok(this)
    }

    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(Duration::from_secs(2))?;
        c.pragma_update(None, "secure_delete", true)?;
        Ok(c)
    }

    fn encrypt(&self, owner: UserId, id: &str, data: Vec<u8>) -> Result<(Vec<u8>, i64), E> {
        match self.key_provider.as_ref() {
            Some(provider) => {
                let context = content_context(owner, id);
                let envelope = seal_field(provider.as_ref(), context.as_bytes(), &data)
                    .map_err(|_| E::Unavailable)?;
                Ok((envelope.encode(), 1))
            }
            None => Ok((data, 0)),
        }
    }

    fn decrypt(&self, owner: UserId, id: &str, data: Vec<u8>) -> Result<Vec<u8>, E> {
        let provider = self.key_provider.as_ref().ok_or(E::Unavailable)?;
        let envelope = FieldEnvelope::decode(&data).map_err(|_| E::Unavailable)?;
        let context = content_context(owner, id);
        let plaintext = open_field(provider.as_ref(), context.as_bytes(), &envelope)
            .map_err(|_| E::Unavailable)?;
        Ok(plaintext.to_vec())
    }
}

fn content_context(owner: UserId, id: &str) -> String {
    format!("personal-file:{owner}:{id}")
}

fn db(_: rusqlite::Error) -> E {
    E::Unavailable
}
fn name_valid(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 255
        && ![".", ".."].contains(&name)
        && !name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
}
fn parent(c: &Connection, u: UserId, id: &str) -> Result<(), E> {
    if id == "root" {
        return Ok(());
    }
    let found: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM personal_files WHERE owner=?1 AND id=?2 AND directory=1)",
            params![u.to_string(), id],
            |r| r.get(0),
        )
        .map_err(db)?;
    if found { Ok(()) } else { Err(E::Missing) }
}
fn duplicate(c: &Connection, u: UserId, p: &str, n: &str, id: &str) -> Result<(), E> {
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM personal_files WHERE owner=?1 AND parent=?2 AND name=?3 AND id<>?4)",params![u.to_string(),p,n,id],|r|r.get(0)).map_err(db)?;
    if exists { Err(E::Conflict) } else { Ok(()) }
}
impl PersonalFiles for SqlitePersonalFiles {
    fn list(&self, u: UserId, p: &str) -> Result<Vec<PersonalFile>, E> {
        let c = self.connect().map_err(db)?;
        parent(&c, u, p)?;
        let mut s=c.prepare("SELECT id,parent,name,directory,size FROM personal_files WHERE owner=?1 AND parent=?2 ORDER BY directory DESC,name,id LIMIT 1001").map_err(db)?;
        s.query_map(params![u.to_string(), p], |r| {
            Ok(PersonalFile {
                id: r.get(0)?,
                parent: r.get(1)?,
                name: r.get(2)?,
                directory: r.get(3)?,
                size: r.get::<_, i64>(4)? as u64,
            })
        })
        .map_err(db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db)
    }
    fn create(&self, u: UserId, p: &str, n: &str, data: Option<Vec<u8>>) -> Result<(), E> {
        if !name_valid(n) {
            return Err(E::Invalid);
        }
        if data.as_ref().is_some_and(|v| v.len() > 16 * 1024 * 1024) {
            return Err(E::Limit);
        }
        let is_directory = data.is_none();
        let mut c = self.connect().map_err(db)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        parent(&tx, u, p)?;
        duplicate(&tx, u, p, n, "")?;
        let plaintext_len = data.as_ref().map_or(0, |v| v.len() as i64);
        let (count, size): (i64, i64) = tx
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(size),0) FROM personal_files WHERE owner=?1",
                [u.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        if count >= 1000 || size + plaintext_len > 128 * 1024 * 1024 {
            return Err(E::Limit);
        }
        let id: String = tx
            .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))
            .map_err(db)?;
        let (stored, encrypted) = match data {
            Some(bytes) => self.encrypt(u, &id, bytes)?,
            None => (Vec::new(), 0),
        };
        tx.execute(
            "INSERT INTO personal_files (owner,id,parent,name,directory,data,size,encrypted) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![u.to_string(), id, p, n, is_directory, stored, plaintext_len, encrypted],
        )
        .map_err(db)?;
        tx.commit().map_err(db)
    }
    fn read(&self, u: UserId, id: &str) -> Result<(String, Vec<u8>), E> {
        let c = self.connect().map_err(db)?;
        let row = c
            .query_row(
                "SELECT name,data,encrypted FROM personal_files WHERE owner=?1 AND id=?2 AND directory=0",
                params![u.to_string(), id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(db)?
            .ok_or(E::Missing)?;
        let (name, data, encrypted) = row;
        let bytes = data.unwrap_or_default();
        let content = if encrypted == 1 {
            self.decrypt(u, id, bytes)?
        } else {
            bytes
        };
        Ok((name, content))
    }
    fn relocate(&self, u: UserId, id: &str, p: &str, n: &str) -> Result<(), E> {
        if !name_valid(n) {
            return Err(E::Invalid);
        }
        let mut c = self.connect().map_err(db)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        parent(&tx, u, p)?;
        duplicate(&tx, u, p, n, id)?;
        let mut ancestor = p.to_owned();
        let mut steps = 0;
        while ancestor != "root" {
            if ancestor == id || steps > 1000 {
                return Err(E::Invalid);
            }
            steps += 1;
            ancestor = tx
                .query_row(
                    "SELECT parent FROM personal_files WHERE owner=?1 AND id=?2",
                    params![u.to_string(), ancestor],
                    |r| r.get(0),
                )
                .map_err(db)?;
        }
        if tx
            .execute(
                "UPDATE personal_files SET parent=?3,name=?4 WHERE owner=?1 AND id=?2",
                params![u.to_string(), id, p, n],
            )
            .map_err(db)?
            == 0
        {
            return Err(E::Missing);
        }
        tx.commit().map_err(db)
    }
    fn delete(&self, u: UserId, id: &str) -> Result<(), E> {
        let mut c = self.connect().map_err(db)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let children: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM personal_files WHERE owner=?1 AND parent=?2)",
                params![u.to_string(), id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if children {
            return Err(E::Conflict);
        }
        if tx
            .execute(
                "DELETE FROM personal_files WHERE owner=?1 AND id=?2",
                params![u.to_string(), id],
            )
            .map_err(db)?
            == 0
        {
            return Err(E::Missing);
        }
        tx.commit().map_err(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::{SecretEncryptionKey, SecretEncryptionKeyId};
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("key error")
        }
    }
    impl Error for TestError {}

    struct KeyProvider;

    impl crate::secret_store::SecretEncryptionKeyProvider for KeyProvider {
        type Error = TestError;

        fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
            Ok(SecretEncryptionKey::new(
                SecretEncryptionKeyId::parse("key-1").unwrap(),
                [7_u8; 32],
            ))
        }

        fn key_by_id(
            &self,
            _id: &SecretEncryptionKeyId,
        ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
            <Self as crate::secret_store::SecretEncryptionKeyProvider>::active_key(self).map(Some)
        }
    }

    #[test]
    fn encrypted_content_is_not_stored_in_plaintext() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("files.sqlite");
        let store = SqlitePersonalFiles::open_encrypted(&path, Arc::new(KeyProvider)).unwrap();
        let user = UserId::new();
        store
            .create(user, "root", "note.txt", Some(b"top secret".to_vec()))
            .unwrap();

        let files = store.list(user, "root").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, 10);
        let raw: Vec<u8> = store
            .connect()
            .unwrap()
            .query_row("SELECT data FROM personal_files", [], |row| row.get(0))
            .unwrap();
        assert!(
            !raw.windows(b"top secret".len())
                .any(|window| window == b"top secret")
        );

        let (name, content) = store.read(user, &files[0].id).unwrap();
        assert_eq!(name, "note.txt");
        assert_eq!(content, b"top secret");
    }

    #[test]
    fn legacy_plaintext_rows_remain_readable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("files.sqlite");
        let user = UserId::new();
        {
            let store = SqlitePersonalFiles::open(&path).unwrap();
            store
                .create(user, "root", "plain.txt", Some(b"plain content".to_vec()))
                .unwrap();
        }
        let store = SqlitePersonalFiles::open_encrypted(&path, Arc::new(KeyProvider)).unwrap();
        let files = store.list(user, "root").unwrap();
        assert_eq!(
            store.read(user, &files[0].id).unwrap().1,
            b"plain content".to_vec()
        );

        store
            .create(user, "root", "secret.txt", Some(b"hidden".to_vec()))
            .unwrap();
        let secret = store
            .list(user, "root")
            .unwrap()
            .into_iter()
            .find(|file| file.name == "secret.txt")
            .unwrap();
        assert_eq!(store.read(user, &secret.id).unwrap().1, b"hidden".to_vec());
    }
}
