use rumahl_core::{FileError as E, PersonalFile, PersonalFiles, UserId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
pub struct SqlitePersonalFiles {
    path: PathBuf,
}
impl SqlitePersonalFiles {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let this = Self {
            path: path.as_ref().into(),
        };
        this.connect()?.execute_batch("PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS personal_files (owner TEXT NOT NULL,id TEXT NOT NULL,parent TEXT NOT NULL,name TEXT NOT NULL,directory INTEGER NOT NULL,data BLOB,PRIMARY KEY(owner,id),UNIQUE(owner,parent,name)); CREATE INDEX IF NOT EXISTS personal_files_parent ON personal_files(owner,parent);")?;
        Ok(this)
    }
    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(Duration::from_secs(2))?;
        c.pragma_update(None, "secure_delete", true)?;
        Ok(c)
    }
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
        let mut s=c.prepare("SELECT id,parent,name,directory,COALESCE(length(data),0) FROM personal_files WHERE owner=?1 AND parent=?2 ORDER BY directory DESC,name,id LIMIT 1001").map_err(db)?;
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
        let mut c = self.connect().map_err(db)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        parent(&tx, u, p)?;
        duplicate(&tx, u, p, n, "")?;
        let (count, size): (i64, i64) = tx
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(length(data)),0) FROM personal_files WHERE owner=?1",
                [u.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        if count >= 1000 || size + data.as_ref().map_or(0, |v| v.len() as i64) > 128 * 1024 * 1024 {
            return Err(E::Limit);
        }
        let id: String = tx
            .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))
            .map_err(db)?;
        tx.execute(
            "INSERT INTO personal_files VALUES (?1,?2,?3,?4,?5,?6)",
            params![u.to_string(), id, p, n, data.is_none(), data],
        )
        .map_err(db)?;
        tx.commit().map_err(db)
    }
    fn read(&self, u: UserId, id: &str) -> Result<(String, Vec<u8>), E> {
        self.connect()
            .map_err(db)?
            .query_row(
                "SELECT name,data FROM personal_files WHERE owner=?1 AND id=?2 AND directory=0",
                params![u.to_string(), id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db)?
            .ok_or(E::Missing)
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
