//! Runtime records: what the supervisor captured about an installed app image.
//!
//! When a container package is imported, the runtime supervisor stages the
//! image and reports the immutable reference plus the image's own
//! configuration environment. The service records that answer next to the
//! app state so the extended configuration view can show it later. The record
//! is diagnostic metadata: it never gates installation, and the runtime keeps
//! resolving images through the supervisor's staged reference, not through
//! this file.

use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use rumahl_core::InstallationId;
use rumahl_platform_buildroot::PublishedInstall;
use rumahl_platform_buildroot::StagedImageEnvironment as ImportEnvironment;
use rumahl_platform_web::{
    AppAccessError, AppRuntimeEnvEntry, AppRuntimeEnvironment, ShellIdentity,
};

use crate::apps::PersistentApps;

/// State-directory layout: one record per installation.
pub(crate) const RUNTIME_RECORDS_DIR: &str = "app-runtimes";
/// Bounded record size; anything larger is corrupt, not truncated.
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const RECORD_DIRECTORY_MODE: u32 = 0o700;
const RECORD_FILE_MODE: u32 = 0o600;

/// The captured environment of one installed container image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeRecord {
    pub image: String,
    pub environment: Vec<(String, String)>,
}

impl RuntimeRecord {
    fn from_published(published: &PublishedInstall) -> Option<Self> {
        let PublishedInstall::Container {
            image, environment, ..
        } = published
        else {
            return None;
        };
        Some(Self {
            image: image.as_str().to_owned(),
            environment: environment
                .iter()
                .map(|entry: &ImportEnvironment| (entry.name.clone(), entry.value.clone()))
                .collect(),
        })
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "image": self.image,
            "environment": self
                .environment
                .iter()
                .map(|(name, value)| serde_json::json!({"name": name, "value": value}))
                .collect::<Vec<_>>(),
        })
    }

    fn from_json(value: serde_json::Value) -> Option<Self> {
        let object = value.as_object()?;
        if object.len() != 3 || object.get("version") != Some(&serde_json::json!(1)) {
            return None;
        }
        let image = object.get("image")?.as_str()?;
        if image.is_empty() || image.len() > 512 {
            return None;
        }
        let entries = object.get("environment")?.as_array()?;
        if entries.len() > 64 {
            return None;
        }
        let mut environment = Vec::with_capacity(entries.len());
        for entry in entries {
            let record = entry.as_object()?;
            if record.len() != 2 {
                return None;
            }
            let name = record.get("name")?.as_str()?;
            let value = record.get("value")?.as_str()?;
            if name.is_empty()
                || name.len() > 128
                || value.len() > 512
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
            {
                return None;
            }
            environment.push((name.to_owned(), value.to_owned()));
        }
        Some(Self {
            image: image.to_owned(),
            environment,
        })
    }
}

fn record_path(state_dir: &Path, installation_id: &InstallationId) -> std::path::PathBuf {
    state_dir
        .join(RUNTIME_RECORDS_DIR)
        .join(format!("{installation_id}.json"))
}

/// Records what the import captured for a container installation. Web and
/// native installs record nothing.
pub(crate) fn write_runtime_record(
    state_dir: &Path,
    installation_id: &InstallationId,
    published: &PublishedInstall,
) -> io::Result<()> {
    let Some(record) = RuntimeRecord::from_published(published) else {
        return Ok(());
    };
    let directory = state_dir.join(RUNTIME_RECORDS_DIR);
    std::fs::create_dir_all(&directory)?;
    std::fs::set_permissions(
        &directory,
        std::fs::Permissions::from_mode(RECORD_DIRECTORY_MODE),
    )?;
    let path = record_path(state_dir, installation_id);
    // Write-through to a temporary file, then move into place: a reader never
    // sees a half-written record.
    let temporary = directory.join(format!(".{installation_id}.json.tmp"));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(RECORD_FILE_MODE)
            .open(&temporary)?;
        use std::io::Write;
        file.write_all(record.to_json().to_string().as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, &path)
}

/// Reads a recorded runtime environment. `None` when nothing was recorded
/// (web apps, native apps, installs from before the record existed) or when
/// the record does not parse: the view reports the absence, never a guess.
pub(crate) fn read_runtime_record(
    state_dir: &Path,
    installation_id: &InstallationId,
) -> Option<RuntimeRecord> {
    let path = record_path(state_dir, installation_id);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || metadata.len() > MAX_RECORD_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    RuntimeRecord::from_json(value)
}

/// Removes a record after the durable uninstall committed. Best effort, like
/// the asset cleanup: a stale record can never keep an app discoverable.
pub(crate) fn remove_runtime_record(state_dir: &Path, installation_id: &InstallationId) {
    let _ = std::fs::remove_file(record_path(state_dir, installation_id));
}

impl PersistentApps {
    /// The recorded runtime environment of an installed app, display-safe:
    /// names that could carry a credential are never reported. The runtime
    /// kind always comes back so the shell can tell "no record yet" from
    /// "nothing to record".
    pub fn app_runtime_environment(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<AppRuntimeEnvironment, AppAccessError> {
        self.authenticate(identity)?;
        let app = self.app(identity, installation)?;
        let runtime = match app.manifest().runtime().kind() {
            rumahl_core::RuntimeKind::Web => "web",
            rumahl_core::RuntimeKind::Container => "container",
            rumahl_core::RuntimeKind::Native => "native",
        };
        let state_dir = self.root.parent().ok_or_else(|| {
            eprintln!("[dbg] root parent");
            AppAccessError::Unavailable
        })?;
        let record = read_runtime_record(state_dir, &installation);
        let mut entries = Vec::new();
        let mut image = None;
        if let Some(record) = record {
            image = Some(record.image);
            for (name, value) in record.environment {
                if !rumahl_platform_web::is_display_safe_name(&name) {
                    continue;
                }
                entries.push(AppRuntimeEnvEntry { name, value });
            }
        }
        Ok(AppRuntimeEnvironment {
            runtime: runtime.to_owned(),
            image,
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rumahl_platform_buildroot::DockerImageReference;

    fn container_install(environment: Vec<(String, String)>) -> PublishedInstall {
        PublishedInstall::Container {
            images: 1,
            image: DockerImageReference::parse(format!("sha256:{}", "a".repeat(64))).unwrap(),
            environment: environment
                .into_iter()
                .map(|(name, value)| ImportEnvironment { name, value })
                .collect(),
        }
    }

    #[test]
    fn roundtrips_a_container_record() {
        let dir = tempfile::tempdir().unwrap();
        let installation = InstallationId::new();
        let published = container_install(vec![
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("RUMAHL_APP_MODE".to_owned(), "batch".to_owned()),
        ]);

        write_runtime_record(dir.path(), &installation, &published).unwrap();

        let record = read_runtime_record(dir.path(), &installation).unwrap();
        assert_eq!(record.image, format!("sha256:{}", "a".repeat(64)));
        assert_eq!(record.environment.len(), 2);
        assert_eq!(
            record.environment[0],
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned())
        );
    }

    #[test]
    fn unknown_installations_have_no_record() {
        let dir = tempfile::tempdir().unwrap();
        let installation = InstallationId::new();
        let published = container_install(vec![]);
        write_runtime_record(dir.path(), &installation, &published).unwrap();

        assert!(read_runtime_record(dir.path(), &InstallationId::new()).is_none());
    }

    #[test]
    fn refuses_symlinked_and_corrupt_records() {
        let dir = tempfile::tempdir().unwrap();
        let installation = InstallationId::new();
        let path = record_path(dir.path(), &installation);
        std::fs::create_dir_all(dir.path().join(RUNTIME_RECORDS_DIR)).unwrap();
        std::fs::write(&path, b"{\"version\":1,\"image\":\"x\",\"environment\":[]").unwrap();

        // Trailing JSON is corrupt and reports absent, never partial data.
        assert!(read_runtime_record(dir.path(), &installation).is_none());

        std::fs::write(&path, b"{}").unwrap();
        assert!(read_runtime_record(dir.path(), &installation).is_none());

        std::fs::remove_file(&path).unwrap();
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, b"{}").unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert!(read_runtime_record(dir.path(), &installation).is_none());
    }

    #[test]
    fn removal_is_best_effort() {
        let dir = tempfile::tempdir().unwrap();
        let installation = InstallationId::new();
        let published = container_install(vec![]);
        write_runtime_record(dir.path(), &installation, &published).unwrap();

        remove_runtime_record(dir.path(), &installation);

        assert!(read_runtime_record(dir.path(), &installation).is_none());
        remove_runtime_record(dir.path(), &installation);
    }
}
