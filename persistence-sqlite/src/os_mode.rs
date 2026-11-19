use rumahl_core::{
    BrowserProfileId, OsMode, OsModeRepository, OsModeSettings, OsModeStoreError as Error,
    PreferenceScope, UserId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const DEVICE_LIMIT: i64 = 128;

/// Stores the operating-system exposure mode.
///
/// Kept separate from `shell_preferences` (presentation only) because the mode is
/// security policy. User and device values use the same revision compare-and-swap
/// as the shell preferences; a device override is the *presence* of a row.
pub struct SqliteOsModeRepository {
    path: PathBuf,
}

impl SqliteOsModeRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let this = Self {
            path: path.as_ref().to_owned(),
        };
        let connection = this.connect()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS os_user_mode (user_id TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision >= 0), os_mode TEXT NOT NULL CHECK(os_mode IN ('guided', 'advanced', 'developer'))); \
             CREATE TABLE IF NOT EXISTS os_device_mode (user_id TEXT NOT NULL, device_id TEXT NOT NULL, os_mode TEXT NOT NULL CHECK(os_mode IN ('guided', 'advanced', 'developer')), PRIMARY KEY(user_id, device_id));",
        )?;
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
    ) -> Result<OsModeSettings, Error> {
        let row = connection
            .query_row(
                "SELECT u.revision, u.os_mode, d.os_mode FROM os_user_mode u LEFT JOIN os_device_mode d ON d.user_id=u.user_id AND d.device_id=?2 WHERE u.user_id=?1",
                params![user.to_string(), device.to_string()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| Error::Unavailable)?;
        let Some((revision, user_mode, device_mode)) = row else {
            return Ok(OsModeSettings::default());
        };
        Ok(OsModeSettings {
            revision: u64::try_from(revision)
                .ok()
                .filter(|value| *value <= 9_007_199_254_740_991)
                .ok_or(Error::Unavailable)?,
            user_mode: OsMode::parse(&user_mode).map_err(|_| Error::Unavailable)?,
            device_mode: device_mode
                .map(|mode| OsMode::parse(&mode).map_err(|_| Error::Unavailable))
                .transpose()?,
        })
    }

    fn count_device_overrides(connection: &Connection, user: UserId) -> Result<i64, Error> {
        connection
            .query_row(
                "SELECT COUNT(*) FROM os_device_mode WHERE user_id=?1",
                [user.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| Error::Unavailable)
    }
}

impl OsModeRepository for SqliteOsModeRepository {
    fn load(&self, user: UserId, device: BrowserProfileId) -> Result<OsModeSettings, Error> {
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
        mode: Option<OsMode>,
    ) -> Result<OsModeSettings, Error> {
        let mut connection = self.connect().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| Error::Unavailable)?;
        let current = Self::read(&tx, user, device)?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        let next = revision
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or(Error::Unavailable)?;
        let user_mode = if scope == PreferenceScope::User {
            mode.unwrap_or(OsMode::Guided)
        } else {
            current.user_mode
        };
        tx.execute(
            "INSERT INTO os_user_mode VALUES (?1,?2,?3) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision,os_mode=excluded.os_mode",
            params![user.to_string(), next as i64, user_mode.as_str()],
        )
        .map_err(|_| Error::Unavailable)?;
        if scope == PreferenceScope::Device {
            match mode {
                Some(mode) => {
                    if current.device_mode.is_none()
                        && Self::count_device_overrides(&tx, user)? >= DEVICE_LIMIT
                    {
                        return Err(Error::Limit);
                    }
                    tx.execute(
                        "INSERT INTO os_device_mode VALUES (?1,?2,?3) ON CONFLICT(user_id,device_id) DO UPDATE SET os_mode=excluded.os_mode",
                        params![user.to_string(), device.to_string(), mode.as_str()],
                    )
                    .map_err(|_| Error::Unavailable)?;
                }
                None => {
                    tx.execute(
                        "DELETE FROM os_device_mode WHERE user_id=?1 AND device_id=?2",
                        params![user.to_string(), device.to_string()],
                    )
                    .map_err(|_| Error::Unavailable)?;
                }
            }
        }
        let result = Self::read(&tx, user, device)?;
        tx.commit().map_err(|_| Error::Unavailable)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: u8) -> BrowserProfileId {
        BrowserProfileId::parse(&format!("00000000-0000-4000-8000-{id:012}")).unwrap()
    }

    fn repo() -> (
        tempfile::TempDir,
        SqliteOsModeRepository,
        UserId,
        BrowserProfileId,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let repo = SqliteOsModeRepository::open(directory.path().join("os_mode.sqlite")).unwrap();
        (directory, repo, UserId::new(), profile(1))
    }

    #[test]
    fn defaults_to_guided() {
        let (_dir, repo, user, device) = repo();
        let settings = repo.load(user, device).unwrap();
        assert_eq!(settings.revision, 0);
        assert_eq!(settings.effective_mode(), OsMode::Guided);
    }

    #[test]
    fn user_and_device_overrides_resolve() {
        let (_dir, repo, user, device) = repo();
        let other = profile(2);

        let saved = repo
            .save(
                user,
                device,
                0,
                PreferenceScope::User,
                Some(OsMode::Advanced),
            )
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.effective_mode(), OsMode::Advanced);

        let saved = repo
            .save(
                user,
                device,
                1,
                PreferenceScope::Device,
                Some(OsMode::Developer),
            )
            .unwrap();
        assert_eq!(saved.effective_mode(), OsMode::Developer);
        assert_eq!(
            repo.load(user, other).unwrap().effective_mode(),
            OsMode::Advanced
        );

        let saved = repo
            .save(user, device, 2, PreferenceScope::Device, None)
            .unwrap();
        assert_eq!(saved.effective_mode(), OsMode::Advanced);
    }

    #[test]
    fn stale_revision_conflicts() {
        let (_dir, repo, user, device) = repo();
        repo.save(
            user,
            device,
            0,
            PreferenceScope::User,
            Some(OsMode::Advanced),
        )
        .unwrap();
        assert_eq!(
            repo.save(
                user,
                device,
                0,
                PreferenceScope::User,
                Some(OsMode::Advanced)
            )
            .unwrap_err(),
            Error::Conflict
        );
    }

    #[test]
    fn user_isolation() {
        let (_dir, repo, user, device) = repo();
        let other_user = UserId::new();
        repo.save(
            user,
            device,
            0,
            PreferenceScope::User,
            Some(OsMode::Developer),
        )
        .unwrap();
        assert_eq!(
            repo.load(other_user, device).unwrap().effective_mode(),
            OsMode::Guided
        );
    }
}
