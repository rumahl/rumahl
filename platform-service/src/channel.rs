//! Production wiring for the persistent app channel.
//!
//! Always-on container apps reach the OS over one Unix socket that every app
//! container has mounted at [`CONTAINER_SOCKET`]. The app authenticates with a
//! per-installation 256-bit token; the token and a small JSON descriptor live
//! in the app's private data directory (`<data>/.rumahl/channel.json`) and are
//! published before the supervisor activates the container, so the app can
//! read them on startup. Only the token store in the state directory decides
//! whether a hello is valid.
//!
//! Files:
//!
//! * `<state>/channel-tokens/<installation>` — authoritative token, `0600`,
//!   stable across restarts and removed on uninstall.
//! * `RUMAHL_CHANNEL_SOCKET` (or `<state>/channel/channel.sock`) — the listener
//!   the supervisor mounts into app containers.
//! * `<data>/.rumahl/channel.json` — what the app reads: protocol, installation
//!   id, container socket path and token, `0600`.
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rumahl_core::{AuditLog, InstallationId, RuntimeChannelRegistry};
use rumahl_platform_web::{ChannelAuthenticator, ChannelFiles, serve_channel_listener};

/// Protocol version the app must announce in its hello.
pub use rumahl_platform_web::CHANNEL_PROTOCOL;
/// Where app containers find the channel socket.
pub const CONTAINER_SOCKET: &str = "/run/rumahl/channel/channel.sock";
/// Directory under the state volume that holds the tokens.
const TOKEN_DIRECTORY: &str = "channel-tokens";
/// Upper bound for the encoded token (43 characters for a 256-bit token).
const MAX_TOKEN_CHARS: usize = 64;
/// App material lives next to the rest of an app's private state.
const MATERIAL_DIRECTORY: &str = ".rumahl";
const MATERIAL_FILE: &str = "channel.json";

/// The state-side token store: issues, persists, authenticates and revokes.
#[derive(Debug)]
pub struct ChannelTokens {
    directory: PathBuf,
}

impl ChannelTokens {
    pub fn in_state(state_dir: &Path) -> Self {
        Self {
            directory: state_dir.join(TOKEN_DIRECTORY),
        }
    }

    fn path(&self, installation: &InstallationId) -> PathBuf {
        self.directory.join(installation.to_string())
    }

    /// Returns the installation's token, issuing one on first use. The token
    /// is stable across restarts so an app keeps working after a reboot.
    pub fn issue(&self, installation: InstallationId) -> io::Result<String> {
        create_private_directory(&self.directory, 0o700)?;
        let path = self.path(&installation);
        if let Some(token) = read_token(&path)? {
            return Ok(token);
        }
        let token = rumahl_account_auth::SessionToken::generate()
            .map_err(|_| io::Error::other("channel token generation failed"))?
            .encode()
            .to_string();
        write_private(&path, token.as_bytes(), 0o600)?;
        Ok(token)
    }

    /// Removes the installation's token; a running app loses the channel on
    /// its next reconnect.
    pub fn revoke(&self, installation: &InstallationId) {
        let _ = fs::remove_file(self.path(installation));
    }
}

impl ChannelAuthenticator for ChannelTokens {
    fn authenticate(&self, installation: &str, token: &str) -> Option<InstallationId> {
        let installation = InstallationId::parse(installation).ok()?;
        let expected = read_token(&self.path(&installation)).ok()??;
        constant_time_eq(expected.as_bytes(), token.as_bytes()).then_some(installation)
    }
}

/// Vocabulary the service needs to start the listener.
pub struct ChannelListener {
    pub socket: PathBuf,
    pub registry: Arc<RuntimeChannelRegistry>,
    pub tokens: Arc<dyn ChannelAuthenticator>,
    pub files: Option<Arc<dyn ChannelFiles>>,
    pub audit: Option<Arc<dyn AuditLog>>,
}

/// The configured socket path: explicit override or a private path under the
/// state volume. The supervisor mounts this path into app containers.
pub fn socket_path(state_dir: &Path) -> PathBuf {
    match std::env::var_os("RUMAHL_CHANNEL_SOCKET") {
        Some(value) => PathBuf::from(value),
        None => state_dir.join("channel").join("channel.sock"),
    }
}

/// Binds the channel socket and serves connections on a background thread.
/// Returns the path that was bound.
pub fn start(listener: ChannelListener) -> io::Result<PathBuf> {
    if !listener.socket.is_absolute() {
        return Err(io::Error::other("channel socket path must be absolute"));
    }
    let parent = listener
        .socket
        .parent()
        .ok_or_else(|| io::Error::other("channel socket has no parent directory"))?;
    create_private_directory(parent, 0o751)?;
    match fs::symlink_metadata(&listener.socket) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            // A stale socket from a previous process is safe to replace.
            fs::remove_file(&listener.socket)?;
        }
        Ok(_) => {
            return Err(io::Error::other(
                "channel socket path is occupied by a non-socket file",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let socket = std::os::unix::net::UnixListener::bind(&listener.socket)?;
    fs::set_permissions(&listener.socket, fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new()
        .name("rumahl-app-channel".to_owned())
        .spawn(move || {
            serve_channel_listener(
                socket,
                listener.registry,
                listener.tokens,
                listener.files,
                listener.audit,
            );
        })?;
    Ok(listener.socket)
}

/// Writes the token descriptor into an app's private data directory. The
/// directory is created owner-only; an existing file is replaced atomically.
pub fn publish_app_material(
    data_dir: &Path,
    installation: InstallationId,
    token: &str,
) -> io::Result<()> {
    let directory = data_dir.join(MATERIAL_DIRECTORY);
    create_private_directory(&directory, 0o700)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(
            "app channel directory is not a plain directory",
        ));
    }
    let material = serde_json::json!({
        "protocol": CHANNEL_PROTOCOL,
        "installationId": installation.to_string(),
        "socket": CONTAINER_SOCKET,
        "token": token,
    });
    write_private(
        &directory.join(MATERIAL_FILE),
        format!("{material}\n").as_bytes(),
        0o600,
    )
}

fn read_token(path: &Path) -> io::Result<Option<String>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut token = String::new();
    file.take((MAX_TOKEN_CHARS + 2) as u64)
        .read_to_string(&mut token)?;
    let token = token.trim();
    if token.is_empty()
        || token.len() > MAX_TOKEN_CHARS
        || rumahl_account_auth::SessionToken::parse(token).is_err()
    {
        return Err(io::Error::other("channel token file is malformed"));
    }
    Ok(Some(token.to_owned()))
}

/// Creates `path` owner-only, refusing a symlink at any component we own.
pub(crate) fn create_private_directory(path: &Path, mode: u32) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(io::Error::other(
                "channel directory path is not a plain directory",
            ));
        }
        Ok(_) => {
            // An existing directory may predate the group-traversal rule.
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn write_private(path: &Path, content: &[u8], mode: u32) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("channel file has no parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("channel file has no name"))?
        .to_string_lossy();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0)
        ^ u128::from(std::process::id());
    let temporary = directory.join(format!(".{name}.tmp-{suffix}"));
    let written = (|| -> io::Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

/// Compares two secrets without leaking where they differ.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    use rumahl_core::{RuntimeChannelRegistry, RuntimeEvent};

    #[test]
    fn tokens_are_stable_private_and_installation_scoped() {
        let temp = tempfile::tempdir().unwrap();
        let tokens = ChannelTokens::in_state(temp.path());
        let installation = InstallationId::new();
        let token = tokens.issue(installation).unwrap();
        assert!(token.len() <= MAX_TOKEN_CHARS);
        // Re-issuing returns the same token, so a restart keeps the app working.
        assert_eq!(tokens.issue(installation).unwrap(), token);
        let path = tokens.path(&installation);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&tokens.directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        assert_eq!(
            tokens.authenticate(&installation.to_string(), &token),
            Some(installation)
        );
        assert_eq!(
            tokens.authenticate(&installation.to_string(), "0".repeat(64).as_str()),
            None
        );
        // Another installation cannot borrow the token.
        assert_eq!(
            tokens.authenticate(&InstallationId::new().to_string(), &token),
            None
        );
        tokens.revoke(&installation);
        assert_eq!(tokens.authenticate(&installation.to_string(), &token), None);
    }

    #[test]
    fn app_material_names_the_container_socket_and_stays_private() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("app-data");
        create_private_directory(&data, 0o750).unwrap();
        let installation = InstallationId::new();
        let token = "a".repeat(64);
        publish_app_material(&data, installation, &token).unwrap();

        let material = data.join(".rumahl/channel.json");
        let mode = fs::metadata(&material).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&material).unwrap()).unwrap();
        assert_eq!(value["protocol"], CHANNEL_PROTOCOL);
        assert_eq!(value["installationId"], installation.to_string());
        assert_eq!(value["socket"], CONTAINER_SOCKET);
        assert_eq!(value["token"], token);

        // A symlinked material directory is refused instead of written through.
        let data = temp.path().join("linked");
        create_private_directory(&data, 0o750).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, data.join(MATERIAL_DIRECTORY)).unwrap();
        assert!(publish_app_material(&data, installation, &token).is_err());
    }

    #[test]
    fn listener_authenticates_an_app_and_delivers_events() {
        let temp = tempfile::tempdir().unwrap();
        let socket_path = temp.path().join("channel/channel.sock");
        let tokens = Arc::new(ChannelTokens::in_state(temp.path()));
        let installation = InstallationId::new();
        let token = tokens.issue(installation).unwrap();
        let registry = Arc::new(RuntimeChannelRegistry::new());
        start(ChannelListener {
            socket: socket_path.clone(),
            registry: Arc::clone(&registry),
            tokens: tokens.clone(),
            files: None,
            audit: None,
        })
        .unwrap();

        let mut client = UnixStream::connect(&socket_path).unwrap();
        let hello = serde_json::json!({
            "kind": "hello",
            "protocol": CHANNEL_PROTOCOL,
            "installationId": installation.to_string(),
            "token": token,
        });
        writeln!(client, "{hello}").unwrap();
        client.flush().unwrap();
        let mut waited = 0;
        while !registry.is_registered(&installation) && waited < 400 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            waited += 1;
        }
        assert!(registry.is_registered(&installation));

        registry
            .deliver(
                &installation,
                &RuntimeEvent {
                    topic: "rumahl.files.changed".into(),
                    resource: None,
                },
            )
            .unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.contains("rumahl.files.changed"));

        // A wrong token never registers a channel.
        let mut rejected = UnixStream::connect(&socket_path).unwrap();
        let hello = serde_json::json!({
            "kind": "hello",
            "protocol": CHANNEL_PROTOCOL,
            "installationId": installation.to_string(),
            "token": "f".repeat(64),
        });
        writeln!(rejected, "{hello}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        // The original channel is still the registered one.
        assert!(registry.is_registered(&installation));
    }

    #[test]
    fn listener_refuses_to_replace_a_regular_file() {
        let temp = tempfile::tempdir().unwrap();
        let socket_path = temp.path().join("channel/channel.sock");
        fs::create_dir_all(socket_path.parent().unwrap()).unwrap();
        fs::write(&socket_path, b"not a socket").unwrap();
        let error = start(ChannelListener {
            socket: socket_path.clone(),
            registry: Arc::new(RuntimeChannelRegistry::new()),
            tokens: Arc::new(ChannelTokens::in_state(temp.path())),
            files: None,
            audit: None,
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(fs::read(&socket_path).unwrap(), b"not a socket");
    }
}
