use rumahl_core::{
    BrowserProfileId, PreferenceScope, ShellMode, ShellPreferences, ShellPreferencesError as Error,
    ShellPreferencesRepository, UserId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// Separate from account/session tables; no credentials are stored here.
pub struct SqliteShellPreferences {
    path: PathBuf,
}
impl SqliteShellPreferences {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let this = Self {
            path: path.as_ref().to_owned(),
        };
        this.connect()?.execute_batch("CREATE TABLE IF NOT EXISTS shell_user_preferences (user_id TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision >= 0), shell_mode TEXT NOT NULL CHECK(shell_mode IN ('desktop', 'launcher'))); CREATE TABLE IF NOT EXISTS shell_device_preferences (user_id TEXT NOT NULL, device_id TEXT NOT NULL, shell_mode TEXT NOT NULL CHECK(shell_mode IN ('desktop', 'launcher')), PRIMARY KEY(user_id, device_id));")?;
        this.connect()?.execute_batch("CREATE TABLE IF NOT EXISTS shell_workspace_revisions (user_id TEXT PRIMARY KEY, revision INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS shell_workspaces (user_id TEXT NOT NULL, profile TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(user_id,profile));")?;
        Ok(this)
    }
    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        Ok(connection)
    }
    fn read(
        connection: &Connection,
        user: UserId,
        device: BrowserProfileId,
    ) -> Result<ShellPreferences, Error> {
        // One SQL statement observes user and device values at the same database revision.
        let row = connection.query_row("SELECT u.revision, u.shell_mode, d.shell_mode FROM shell_user_preferences u LEFT JOIN shell_device_preferences d ON d.user_id=u.user_id AND d.device_id=?2 WHERE u.user_id=?1", params![user.to_string(), device.to_string()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))).optional().map_err(|_| Error::Unavailable)?;
        let Some((revision, user, device)) = row else {
            return Ok(ShellPreferences::default());
        };
        Ok(ShellPreferences {
            revision: u64::try_from(revision)
                .ok()
                .filter(|v| *v <= 9_007_199_254_740_991)
                .ok_or(Error::Unavailable)?,
            user_mode: ShellMode::parse(&user).ok_or(Error::Unavailable)?,
            device_mode: device
                .map(|mode| ShellMode::parse(&mode).ok_or(Error::Unavailable))
                .transpose()?,
        })
    }
}
impl ShellPreferencesRepository for SqliteShellPreferences {
    fn load(&self, user: UserId, device: BrowserProfileId) -> Result<ShellPreferences, Error> {
        Self::read(
            &self.connect().map_err(|_| Error::Unavailable)?,
            user,
            device,
        )
    }
    fn save(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        mode: Option<ShellMode>,
    ) -> Result<ShellPreferences, Error> {
        let mut connection = self.connect().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| Error::Unavailable)?;
        let current = Self::read(&tx, user, device)?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        // JSON clients use safe integers; prevent wraparound and ambiguous revisions.
        let next = revision
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or(Error::Unavailable)?;
        if scope == PreferenceScope::Device && mode.is_some() && current.device_mode.is_none() {
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM shell_device_preferences WHERE user_id=?1",
                    [user.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| Error::Unavailable)?;
            if count >= 128 {
                return Err(Error::Limit);
            }
        }
        let user_mode = if scope == PreferenceScope::User {
            mode.unwrap_or(ShellMode::Desktop)
        } else {
            current.user_mode
        };
        tx.execute("INSERT INTO shell_user_preferences VALUES (?1,?2,?3) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision,shell_mode=excluded.shell_mode", params![user.to_string(), next as i64, user_mode.as_str()]).map_err(|_| Error::Unavailable)?;
        if scope == PreferenceScope::Device {
            if let Some(mode) = mode {
                tx.execute("INSERT INTO shell_device_preferences VALUES (?1,?2,?3) ON CONFLICT(user_id,device_id) DO UPDATE SET shell_mode=excluded.shell_mode", params![user.to_string(), device.to_string(), mode.as_str()]).map_err(|_| Error::Unavailable)?;
            } else {
                tx.execute(
                    "DELETE FROM shell_device_preferences WHERE user_id=?1 AND device_id=?2",
                    params![user.to_string(), device.to_string()],
                )
                .map_err(|_| Error::Unavailable)?;
            }
        }
        let result = Self::read(&tx, user, device)?;
        tx.commit().map_err(|_| Error::Unavailable)?;
        Ok(result)
    }
}

impl rumahl_core::WorkspaceRepository for SqliteShellPreferences {
    fn load_workspace(
        &self,
        user: UserId,
        device: BrowserProfileId,
    ) -> Result<rumahl_core::WorkspacePreferences, Error> {
        read_workspace(
            &self.connect().map_err(|_| Error::Unavailable)?,
            user,
            device,
        )
    }
    fn save_workspace(
        &self,
        user: UserId,
        device: BrowserProfileId,
        revision: u64,
        scope: PreferenceScope,
        value: Option<String>,
    ) -> Result<rumahl_core::WorkspacePreferences, Error> {
        if value.as_ref().is_some_and(|v| v.len() > 65536) {
            return Err(Error::Limit);
        }
        let mut connection = self.connect().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| Error::Unavailable)?;
        let current = read_workspace(&tx, user, device)?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        let next = revision
            .checked_add(1)
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or(Error::Unavailable)?;
        let key = if scope == PreferenceScope::User {
            String::new()
        } else {
            device.to_string()
        };
        if value.is_some() {
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM shell_workspaces WHERE user_id=?1 AND profile<>?2",
                    params![user.to_string(), key],
                    |r| r.get(0),
                )
                .map_err(|_| Error::Unavailable)?;
            if count >= 129 {
                return Err(Error::Limit);
            }
            tx.execute("INSERT INTO shell_workspaces VALUES (?1,?2,?3) ON CONFLICT(user_id,profile) DO UPDATE SET value=excluded.value", params![user.to_string(), key, value]).map_err(|_| Error::Unavailable)?;
        } else {
            tx.execute(
                "DELETE FROM shell_workspaces WHERE user_id=?1 AND profile=?2",
                params![user.to_string(), key],
            )
            .map_err(|_| Error::Unavailable)?;
        }
        tx.execute("INSERT INTO shell_workspace_revisions VALUES (?1,?2) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision", params![user.to_string(), next as i64]).map_err(|_| Error::Unavailable)?;
        let result = read_workspace(&tx, user, device)?;
        tx.commit().map_err(|_| Error::Unavailable)?;
        Ok(result)
    }
}
fn read_workspace(
    connection: &Connection,
    user: UserId,
    device: BrowserProfileId,
) -> Result<rumahl_core::WorkspacePreferences, Error> {
    connection.query_row("SELECT revision, (SELECT value FROM shell_workspaces WHERE user_id=?1 AND profile=''), (SELECT value FROM shell_workspaces WHERE user_id=?1 AND profile=?2) FROM shell_workspace_revisions WHERE user_id=?1", params![user.to_string(), device.to_string()], |r| Ok(rumahl_core::WorkspacePreferences { revision: r.get::<_, i64>(0)? as u64, user: r.get(1)?, device: r.get(2)? })).optional().map(|v| v.unwrap_or_default()).map_err(|_| Error::Unavailable)
}
