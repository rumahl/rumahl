//! SQLite adapter for the OS severe-weather warning service.

use rumahl_core::{
    AppId, MAX_ACTIVE_WARNINGS, MAX_WARNINGS_PER_SOURCE, WarningLevel, WarningLogAction,
    WarningSource, WeatherWarning, WeatherWarningDraft, WeatherWarningError,
    WeatherWarningLogEntry, WeatherWarningRepository, valid_warning_source_name,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex, time::Duration};

/// Active warning cache and the warning log share the platform database.
pub struct SqliteWeatherWarnings {
    connection: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredWarning {
    id: String,
    level: String,
    headline: String,
    description: String,
    region: String,
    event: String,
    severity: String,
    sender: String,
    instruction: String,
    area: String,
    effective_at_ms: Option<i64>,
    expires_at_ms: i64,
    test: bool,
    published_at_ms: i64,
    updated_at_ms: i64,
}

impl StoredWarning {
    fn from_warning(warning: &WeatherWarning) -> Self {
        Self {
            id: warning.id.clone(),
            level: warning.level.as_str().to_owned(),
            headline: warning.headline.clone(),
            description: warning.description.clone(),
            region: warning.region.clone(),
            event: warning.event.clone(),
            severity: warning.severity.clone(),
            sender: warning.sender.clone(),
            instruction: warning.instruction.clone(),
            area: warning.area.clone(),
            effective_at_ms: warning.effective_at_ms,
            expires_at_ms: warning.expires_at_ms,
            test: warning.test,
            published_at_ms: warning.published_at_ms,
            updated_at_ms: warning.updated_at_ms,
        }
    }

    fn into_warning(self, source: WarningSource) -> Result<WeatherWarning, WeatherWarningError> {
        let level = WarningLevel::parse(&self.level).ok_or(WeatherWarningError::Unavailable)?;
        Ok(WeatherWarning {
            id: self.id,
            source,
            level,
            headline: self.headline,
            description: self.description,
            region: self.region,
            event: self.event,
            severity: self.severity,
            sender: self.sender,
            instruction: self.instruction,
            area: self.area,
            effective_at_ms: self.effective_at_ms,
            expires_at_ms: self.expires_at_ms,
            test: self.test,
            published_at_ms: self.published_at_ms,
            updated_at_ms: self.updated_at_ms,
        })
    }
}

impl SqliteWeatherWarnings {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, rusqlite::Error> {
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS weather_warning_source (
                 source_id TEXT PRIMARY KEY,
                 display_name TEXT NOT NULL,
                 registered_at INTEGER NOT NULL CHECK (registered_at >= 0),
                 updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
             );
             CREATE TABLE IF NOT EXISTS weather_warning (
                 source_id TEXT NOT NULL,
                 warning_id TEXT NOT NULL,
                 payload TEXT NOT NULL,
                 level TEXT NOT NULL,
                 headline TEXT NOT NULL,
                 expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
                 test INTEGER NOT NULL DEFAULT 0 CHECK (test IN (0, 1)),
                 published_at INTEGER NOT NULL CHECK (published_at >= 0),
                 updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
                 PRIMARY KEY (source_id, warning_id),
                 FOREIGN KEY (source_id) REFERENCES weather_warning_source(source_id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS weather_warning_expiry
                 ON weather_warning(expires_at);
             CREATE TABLE IF NOT EXISTS weather_warning_log (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 source_id TEXT NOT NULL,
                 warning_id TEXT NOT NULL,
                 action TEXT NOT NULL,
                 level TEXT NOT NULL,
                 headline TEXT NOT NULL,
                 started_at INTEGER NOT NULL CHECK (started_at >= 0),
                 ended_at INTEGER,
                 expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
                 test INTEGER NOT NULL DEFAULT 0 CHECK (test IN (0, 1)),
                 attributes_json TEXT NOT NULL DEFAULT '{}'
             );
             CREATE INDEX IF NOT EXISTS weather_warning_log_warning
                 ON weather_warning_log(source_id, warning_id, sequence);
             CREATE INDEX IF NOT EXISTS weather_warning_log_open
                 ON weather_warning_log(ended_at);",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connect(&self) -> Result<std::sync::MutexGuard<'_, Connection>, WeatherWarningError> {
        self.connection
            .lock()
            .map_err(|_| WeatherWarningError::Unavailable)
    }

    fn read_source(
        connection: &Connection,
        app_id: &str,
    ) -> Result<Option<WarningSource>, WeatherWarningError> {
        connection
            .query_row(
                "SELECT source_id, display_name, registered_at, updated_at
                 FROM weather_warning_source WHERE source_id = ?1",
                [app_id],
                |row| {
                    Ok(WarningSource {
                        app_id: row.get(0)?,
                        display_name: row.get(1)?,
                        registered_at_ms: row.get(2)?,
                        updated_at_ms: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|_| WeatherWarningError::Unavailable)
    }

    /// Closes log periods for warnings that expired and removes their cache row.
    fn purge_expired(connection: &Connection, now_ms: i64) -> Result<(), WeatherWarningError> {
        connection
            .execute(
                "UPDATE weather_warning_log SET ended_at = expires_at
                 WHERE ended_at IS NULL AND expires_at <= ?1",
                [now_ms],
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        connection
            .execute(
                "DELETE FROM weather_warning WHERE expires_at <= ?1",
                [now_ms],
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        Ok(())
    }

    fn count(
        connection: &Connection,
        source_id: Option<&str>,
        now_ms: i64,
    ) -> Result<i64, WeatherWarningError> {
        match source_id {
            Some(source_id) => connection
                .query_row(
                    "SELECT COUNT(*) FROM weather_warning WHERE source_id = ?1 AND expires_at > ?2",
                    params![source_id, now_ms],
                    |row| row.get(0),
                )
                .map_err(|_| WeatherWarningError::Unavailable),
            None => connection
                .query_row(
                    "SELECT COUNT(*) FROM weather_warning WHERE expires_at > ?1",
                    [now_ms],
                    |row| row.get(0),
                )
                .map_err(|_| WeatherWarningError::Unavailable),
        }
    }

    fn existing(
        connection: &Connection,
        source_id: &str,
        warning_id: &str,
    ) -> Result<Option<(String, i64)>, WeatherWarningError> {
        connection
            .query_row(
                "SELECT payload, updated_at FROM weather_warning
                 WHERE source_id = ?1 AND warning_id = ?2",
                params![source_id, warning_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| WeatherWarningError::Unavailable)
    }
}

impl WeatherWarningRepository for SqliteWeatherWarnings {
    fn register_source(
        &self,
        app_id: &str,
        display_name: &str,
        now_ms: i64,
    ) -> Result<WarningSource, WeatherWarningError> {
        AppId::parse(app_id).map_err(|_| WeatherWarningError::Invalid)?;
        if !valid_warning_source_name(display_name) {
            return Err(WeatherWarningError::Invalid);
        }
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WeatherWarningError::Unavailable)?;
        tx.execute(
            "INSERT INTO weather_warning_source(source_id, display_name, registered_at, updated_at)
             VALUES(?1, ?2, ?3, ?3)
             ON CONFLICT(source_id) DO UPDATE SET
               display_name = excluded.display_name,
               updated_at = excluded.updated_at",
            params![app_id, display_name.trim(), now_ms.max(0)],
        )
        .map_err(|_| WeatherWarningError::Unavailable)?;
        let source = Self::read_source(&tx, app_id)?.ok_or(WeatherWarningError::Unavailable)?;
        tx.commit().map_err(|_| WeatherWarningError::Unavailable)?;
        Ok(source)
    }

    fn source(&self, app_id: &str) -> Result<Option<WarningSource>, WeatherWarningError> {
        AppId::parse(app_id).map_err(|_| WeatherWarningError::Invalid)?;
        let connection = self.connect()?;
        Self::read_source(&connection, app_id)
    }

    fn publish(
        &self,
        draft: WeatherWarningDraft,
        now_ms: i64,
    ) -> Result<WeatherWarning, WeatherWarningError> {
        let draft = draft.validate(now_ms)?;
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WeatherWarningError::Unavailable)?;
        let source =
            Self::read_source(&tx, &draft.source_id)?.ok_or(WeatherWarningError::UnknownSource)?;
        Self::purge_expired(&tx, now_ms)?;
        let previous = Self::existing(&tx, &draft.source_id, &draft.id)?;
        let action = if previous.is_some() {
            WarningLogAction::Updated
        } else {
            WarningLogAction::Published
        };
        if previous.is_none() {
            if Self::count(&tx, None, now_ms)? >= MAX_ACTIVE_WARNINGS as i64 {
                return Err(WeatherWarningError::Limit);
            }
            if Self::count(&tx, Some(&draft.source_id), now_ms)? >= MAX_WARNINGS_PER_SOURCE as i64 {
                return Err(WeatherWarningError::Limit);
            }
        }
        let published_at_ms = previous
            .as_ref()
            .and_then(|(payload, _)| serde_json::from_str::<StoredWarning>(payload).ok())
            .map_or(now_ms, |warning| warning.published_at_ms);
        let warning = WeatherWarning {
            id: draft.id,
            source: source.clone(),
            level: draft.level,
            headline: draft.headline,
            description: draft.description,
            region: draft.region,
            event: draft.event,
            severity: draft.severity,
            sender: draft.sender,
            instruction: draft.instruction,
            area: draft.area,
            effective_at_ms: draft.effective_at_ms,
            expires_at_ms: draft.expires_at_ms,
            test: draft.test,
            published_at_ms,
            updated_at_ms: now_ms,
        };
        let stored = StoredWarning::from_warning(&warning);
        let payload =
            serde_json::to_string(&stored).map_err(|_| WeatherWarningError::Unavailable)?;
        if previous.is_some() {
            tx.execute(
                "UPDATE weather_warning_log SET ended_at = ?3
                 WHERE source_id = ?1 AND warning_id = ?2 AND ended_at IS NULL",
                params![warning.source.app_id, warning.id, now_ms],
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        }
        tx.execute(
            "INSERT INTO weather_warning_log(
                 source_id, warning_id, action, level, headline, started_at,
                 expires_at, test, attributes_json
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                warning.source.app_id,
                warning.id,
                action.as_str(),
                warning.level.as_str(),
                warning.headline,
                now_ms,
                warning.expires_at_ms,
                i64::from(warning.test),
                payload
            ],
        )
        .map_err(|_| WeatherWarningError::Unavailable)?;
        tx.execute(
            "INSERT INTO weather_warning(
                 source_id, warning_id, payload, level, headline,
                 expires_at, test, published_at, updated_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(source_id, warning_id) DO UPDATE SET
               payload = excluded.payload,
               level = excluded.level,
               headline = excluded.headline,
               expires_at = excluded.expires_at,
               test = excluded.test,
               updated_at = excluded.updated_at",
            params![
                warning.source.app_id,
                warning.id,
                payload,
                warning.level.as_str(),
                warning.headline,
                warning.expires_at_ms,
                i64::from(warning.test),
                warning.published_at_ms,
                warning.updated_at_ms
            ],
        )
        .map_err(|_| WeatherWarningError::Unavailable)?;
        tx.commit().map_err(|_| WeatherWarningError::Unavailable)?;
        Ok(warning)
    }

    fn clear(
        &self,
        source_id: &str,
        ids: Option<&[String]>,
        now_ms: i64,
    ) -> Result<u32, WeatherWarningError> {
        AppId::parse(source_id).map_err(|_| WeatherWarningError::Invalid)?;
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WeatherWarningError::Unavailable)?;
        Self::purge_expired(&tx, now_ms)?;
        let mut removed = 0_u32;
        match ids {
            Some(ids) => {
                for id in ids {
                    if id.is_empty()
                        || id.len() > 64
                        || !id.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric()
                                || matches!(byte, b'.' | b'_' | b':' | b'-')
                        })
                    {
                        return Err(WeatherWarningError::Invalid);
                    }
                    tx.execute(
                        "UPDATE weather_warning_log SET ended_at = ?3
                         WHERE source_id = ?1 AND warning_id = ?2 AND ended_at IS NULL",
                        params![source_id, id, now_ms],
                    )
                    .map_err(|_| WeatherWarningError::Unavailable)?;
                    let count = tx
                        .execute(
                            "DELETE FROM weather_warning WHERE source_id = ?1 AND warning_id = ?2",
                            params![source_id, id],
                        )
                        .map_err(|_| WeatherWarningError::Unavailable)?;
                    removed = removed.saturating_add(count as u32);
                }
            }
            None => {
                tx.execute(
                    "UPDATE weather_warning_log SET ended_at = ?2
                     WHERE source_id = ?1 AND ended_at IS NULL",
                    params![source_id, now_ms],
                )
                .map_err(|_| WeatherWarningError::Unavailable)?;
                let count = tx
                    .execute(
                        "DELETE FROM weather_warning WHERE source_id = ?1",
                        [source_id],
                    )
                    .map_err(|_| WeatherWarningError::Unavailable)?;
                removed = count as u32;
            }
        }
        tx.commit().map_err(|_| WeatherWarningError::Unavailable)?;
        Ok(removed)
    }

    fn active(&self, now_ms: i64) -> Result<Vec<WeatherWarning>, WeatherWarningError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT w.payload, s.source_id, s.display_name, s.registered_at, s.updated_at
                 FROM weather_warning w
                 JOIN weather_warning_source s ON s.source_id = w.source_id
                 WHERE w.expires_at > ?1
                 ORDER BY
                   CASE w.level
                     WHEN 'emergency' THEN 3
                     WHEN 'critical' THEN 2
                     WHEN 'warning' THEN 1
                     ELSE 0
                   END DESC,
                   w.expires_at ASC,
                   w.warning_id ASC",
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        let rows = statement
            .query_map([now_ms], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    WarningSource {
                        app_id: row.get(1)?,
                        display_name: row.get(2)?,
                        registered_at_ms: row.get(3)?,
                        updated_at_ms: row.get(4)?,
                    },
                ))
            })
            .map_err(|_| WeatherWarningError::Unavailable)?;
        let mut warnings = Vec::new();
        for row in rows {
            let (payload, source) = row.map_err(|_| WeatherWarningError::Unavailable)?;
            let stored: StoredWarning =
                serde_json::from_str(&payload).map_err(|_| WeatherWarningError::Unavailable)?;
            let warning = stored.into_warning(source)?;
            if warning.is_active_at(now_ms) {
                warnings.push(warning);
            }
        }
        Ok(warnings)
    }

    fn history(&self, limit: usize) -> Result<Vec<WeatherWarningLogEntry>, WeatherWarningError> {
        let limit = i64::try_from(limit.clamp(1, 200)).map_err(|_| WeatherWarningError::Invalid)?;
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT sequence, source_id, warning_id, action, level, headline,
                        started_at, ended_at, expires_at, test
                 FROM weather_warning_log
                 ORDER BY sequence DESC LIMIT ?1",
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        let rows = statement
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(|_| WeatherWarningError::Unavailable)?;
        let mut history = Vec::new();
        for row in rows {
            let (
                sequence,
                source_id,
                warning_id,
                action,
                level,
                headline,
                started_at,
                ended_at,
                expires_at,
                test,
            ) = row.map_err(|_| WeatherWarningError::Unavailable)?;
            history.push(WeatherWarningLogEntry {
                sequence: u64::try_from(sequence)
                    .ok()
                    .filter(|value| *value <= 9_007_199_254_740_991)
                    .ok_or(WeatherWarningError::Unavailable)?,
                source_id,
                warning_id,
                action: WarningLogAction::parse(&action).ok_or(WeatherWarningError::Unavailable)?,
                level: WarningLevel::parse(&level).ok_or(WeatherWarningError::Unavailable)?,
                headline,
                started_at_ms: started_at,
                ended_at_ms: ended_at,
                expires_at_ms: expires_at,
                test: test != 0,
            });
        }
        Ok(history)
    }

    fn forget_source(&self, app_id: &str, now_ms: i64) -> Result<u32, WeatherWarningError> {
        let removed = self.clear(app_id, None, now_ms)?;
        let connection = self.connect()?;
        connection
            .execute(
                "DELETE FROM weather_warning_source WHERE source_id = ?1",
                [app_id],
            )
            .map_err(|_| WeatherWarningError::Unavailable)?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    fn draft(id: &str, level: WarningLevel, expires: i64) -> WeatherWarningDraft {
        WeatherWarningDraft {
            source_id: "com.example.weather".to_owned(),
            id: id.to_owned(),
            level,
            headline: "Severe storm".to_owned(),
            description: "Gusts up to 120 km/h.".to_owned(),
            region: "Berlin".to_owned(),
            event: "STORM".to_owned(),
            severity: "Severe".to_owned(),
            sender: "DWD".to_owned(),
            instruction: "Secure loose objects.".to_owned(),
            area: "Berlin".to_owned(),
            effective_at_ms: Some(NOW),
            expires_at_ms: expires,
            test: false,
        }
    }

    fn store() -> SqliteWeatherWarnings {
        let store = SqliteWeatherWarnings::open_in_memory().unwrap();
        store
            .register_source("com.example.weather", "Weather", NOW)
            .unwrap();
        store
    }

    #[test]
    fn sources_register_idempotently() {
        let store = store();
        let source = store
            .register_source("com.example.weather", "Weather Pro", NOW + 5)
            .unwrap();
        assert_eq!(source.display_name, "Weather Pro");
        assert_eq!(source.registered_at_ms, NOW);
        assert_eq!(source.updated_at_ms, NOW + 5);
        assert_eq!(
            store.source("com.example.weather").unwrap().unwrap(),
            source
        );
        assert_eq!(
            store.register_source("../bad", "Weather", NOW),
            Err(WeatherWarningError::Invalid)
        );
    }

    #[test]
    fn publishes_updates_and_lists_most_urgent_first() {
        let store = store();
        store
            .publish(draft("info", WarningLevel::Info, NOW + 60_000), NOW)
            .unwrap();
        store
            .publish(
                draft("emergency", WarningLevel::Emergency, NOW + 120_000),
                NOW,
            )
            .unwrap();
        let active = store.active(NOW).unwrap();
        assert_eq!(
            active.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(),
            ["emergency", "info"]
        );
        assert_eq!(active[0].source.display_name, "Weather");

        let updated = store
            .publish(
                WeatherWarningDraft {
                    headline: "Updated storm".to_owned(),
                    ..draft("info", WarningLevel::Critical, NOW + 90_000)
                },
                NOW + 10,
            )
            .unwrap();
        assert_eq!(updated.headline, "Updated storm");
        assert_eq!(updated.published_at_ms, NOW);
        assert_eq!(updated.updated_at_ms, NOW + 10);
        let history = store.history(10).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].action, WarningLogAction::Updated);
        assert!(history.iter().any(|entry| {
            entry.warning_id == "info" && entry.action == WarningLogAction::Published
        }));
        assert!(history.iter().any(|entry| {
            entry.warning_id == "info"
                && entry.action == WarningLogAction::Published
                && entry.ended_at_ms == Some(NOW + 10)
        }));
    }

    #[test]
    fn isolates_sources_and_clears_only_owned_warnings() {
        let store = store();
        store
            .register_source("com.example.other", "Other", NOW)
            .unwrap();
        store
            .publish(draft("mine", WarningLevel::Warning, NOW + 60_000), NOW)
            .unwrap();
        store
            .publish(
                WeatherWarningDraft {
                    source_id: "com.example.other".to_owned(),
                    ..draft("theirs", WarningLevel::Warning, NOW + 60_000)
                },
                NOW,
            )
            .unwrap();
        assert_eq!(
            store
                .clear("com.example.weather", Some(&["theirs".to_owned()]), NOW + 1)
                .unwrap(),
            0
        );
        assert_eq!(store.active(NOW + 1).unwrap().len(), 2);
        assert_eq!(
            store.clear("com.example.weather", None, NOW + 2).unwrap(),
            1
        );
        let active = store.active(NOW + 2).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "theirs");
    }

    #[test]
    fn expiry_removes_warnings_and_closes_log_periods() {
        let store = store();
        store
            .publish(draft("short", WarningLevel::Warning, NOW + 10), NOW)
            .unwrap();
        assert_eq!(store.active(NOW).unwrap().len(), 1);
        assert!(store.active(NOW + 10).unwrap().is_empty());
        store
            .publish(draft("next", WarningLevel::Info, NOW + 100), NOW + 11)
            .unwrap();
        let history = store.history(10).unwrap();
        let short = history
            .iter()
            .find(|entry| entry.warning_id == "short")
            .unwrap();
        assert_eq!(short.ended_at_ms, Some(NOW + 10));
        assert!(store.source("com.example.weather").unwrap().is_some());
    }

    #[test]
    fn enforces_source_and_global_limits() {
        let store = store();
        for index in 0..16 {
            store
                .publish(
                    draft(&format!("w{index}"), WarningLevel::Info, NOW + 60_000),
                    NOW,
                )
                .unwrap();
        }
        assert_eq!(
            store.publish(draft("overflow", WarningLevel::Info, NOW + 60_000), NOW),
            Err(WeatherWarningError::Limit)
        );
    }

    #[test]
    fn forget_source_removes_warnings_and_registration() {
        let store = store();
        store
            .publish(draft("mine", WarningLevel::Warning, NOW + 60_000), NOW)
            .unwrap();
        assert_eq!(
            store.forget_source("com.example.weather", NOW + 1).unwrap(),
            1
        );
        assert!(store.active(NOW + 1).unwrap().is_empty());
        assert_eq!(store.source("com.example.weather").unwrap(), None);
    }
}
