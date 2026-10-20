//! Authorized installed-app catalog and short-lived, installation-bound asset leases.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::ShellIdentity;
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode},
    response::Response,
};
use rand::RngCore;
use rumahl_core::{AppId, InstallationId, PackagePath};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogApp {
    pub id: String,
    pub installation_id: InstallationId,
    pub title: String,
    pub version: String,
    pub launchable: bool,
    /// Permission ids the installed app's manifest declares, exposed so the
    /// shell can gate the app <-> OS bridge per method.
    pub capabilities: Vec<String>,
    /// Runtime lifecycle: `"always-on"` (service) or `"on-demand"`.
    pub lifecycle: String,
    /// External services this app ships a connector bundle for.
    pub connectors: Vec<String>,
}
/// One rendered setting declared by an app manifest, typed for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingInfo {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    /// `text`, `number`, `boolean` or `select`.
    pub kind: String,
    pub required: bool,
    pub options: Vec<AppSettingOptionInfo>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingOptionInfo {
    pub value: String,
    pub label: String,
}
/// The manifest-declared settings for one installed app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingsInfo {
    pub app: CatalogApp,
    pub manifest: Vec<AppSettingInfo>,
}
/// One entry of an app's private data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDataEntry {
    pub name: String,
    pub directory: bool,
    pub size: u64,
}
/// A read-only view of an app's private data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppData {
    Directory(Vec<AppDataEntry>),
    File {
        bytes: Vec<u8>,
        content_type: &'static str,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAccessError {
    Denied,
    Unavailable,
}
/// The runtime activation state of an installed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Running,
    Stopped,
}
impl RuntimeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}
/// A runtime control action requested for an installed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAction {
    Status,
    Start,
    Stop,
}
/// One file of a signed directory package, already base64-decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageUploadFile {
    pub path: String,
    pub bytes: Vec<u8>,
}
/// The public result of importing a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedApp {
    pub id: String,
    pub installation_id: InstallationId,
    pub title: String,
    pub version: String,
}
pub struct AppAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}
/// A resource reference supplied with a capability invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResource {
    pub namespace: String,
    pub kind: String,
    pub key: String,
}
/// The public result of a capability invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityOutcome {
    Invoked,
    Denied,
}
impl CapabilityOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invoked => "invoked",
            Self::Denied => "denied",
        }
    }
}
/// A web provider that must be reached through the shell hosting its iframe
/// (it has no live backend channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserDelivery {
    pub app_id: String,
    pub installation_id: String,
}
/// The outcome of a capability invocation plus the provider's result payload
/// (raw JSON text) when the provider returned one, or a browser delivery
/// instruction when the provider is a running web app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResult {
    pub outcome: CapabilityOutcome,
    pub result: Option<String>,
    pub browser: Option<BrowserDelivery>,
}
/// Every method must re-read installation, grants and live account/session state.
pub trait AppProvider: Send + Sync + 'static {
    fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError>;
    fn entrypoint(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<String, AppAccessError>;
    fn asset(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppAsset, AppAccessError>;
    /// Manifest-declared settings for an installed app, in declaration order.
    fn settings(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
    ) -> Result<Vec<AppSettingInfo>, AppAccessError>;
    /// Read-only listing or contents of an app's private data directory.
    fn data(
        &self,
        identity: ShellIdentity,
        installation: InstallationId,
        path: &str,
    ) -> Result<AppData, AppAccessError>;
    /// Runtime activation state. Fails closed when no runtime control is wired.
    fn runtime_status(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Starts an app runtime (used for on-demand apps and manual control).
    fn start_runtime(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Stops an app runtime while retaining its prepared namespace.
    fn stop_runtime(
        &self,
        _identity: ShellIdentity,
        _installation: InstallationId,
    ) -> Result<RuntimeState, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Verifies and installs a signed package uploaded from the shell. Fails
    /// closed when no importer is wired.
    fn import_package(
        &self,
        _identity: ShellIdentity,
        _files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
    /// Authorizes and delivers a capability invocation to the providing app.
    /// Fails closed when no dispatcher is wired.
    fn invoke_capability(
        &self,
        _identity: ShellIdentity,
        _capability: String,
        _resource: Option<CapabilityResource>,
    ) -> Result<CapabilityResult, AppAccessError> {
        Err(AppAccessError::Unavailable)
    }
}
#[derive(Clone)]
struct Lease {
    identity: ShellIdentity,
    installation: InstallationId,
    expires: Instant,
}
/// Resolves the dynamic port a hosted app uses for IP-based access.
pub trait AppPortProvider: Send + Sync + 'static {
    fn ensure_port(&self, installation: &str) -> Option<u16>;
    fn port_for(&self, installation: &str) -> Option<u16>;
    fn installation_for_port(&self, port: u16) -> Option<String>;
    fn all_ports(&self) -> Vec<u16>;
}

pub struct AppAccess {
    provider: Arc<dyn AppProvider>,
    shell_origins: Vec<String>,
    device_hosts: Vec<String>,
    suffixes: Vec<String>,
    port: Option<u16>,
    ports: Option<Arc<dyn AppPortProvider>>,
    leases: Mutex<HashMap<String, Lease>>,
}
pub struct AppLaunch {
    pub app: CatalogApp,
    pub lease: String,
    pub frame_url: String,
}
impl AppAccess {
    pub fn new(
        provider: Arc<dyn AppProvider>,
        primary_origin: &Url,
        shell_origins: &[Url],
        suffixes: &[String],
        ports: Option<Arc<dyn AppPortProvider>>,
    ) -> Result<Self, AppAccessError> {
        if primary_origin.scheme() != "https"
            || suffixes.is_empty()
            || suffixes.iter().any(|suffix| {
                let valid = suffix.len() <= 180
                    && suffix.contains('.')
                    && suffix.split('.').all(|label| {
                        !label.is_empty()
                            && label.len() <= 63
                            && !label.starts_with('-')
                            && !label.ends_with('-')
                            && label
                                .bytes()
                                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    });
                !valid || primary_origin.host_str() == Some(suffix.as_str())
            })
        {
            return Err(AppAccessError::Unavailable);
        }
        let mut origins: Vec<String> = shell_origins
            .iter()
            .map(|origin| origin.origin().ascii_serialization())
            .collect();
        if origins.is_empty() {
            origins.push(primary_origin.origin().ascii_serialization());
        }
        origins.sort();
        origins.dedup();
        let mut unique_suffixes: Vec<String> = Vec::new();
        for suffix in suffixes {
            if !unique_suffixes.contains(suffix) {
                unique_suffixes.push(suffix.clone());
            }
        }
        let device_hosts: Vec<String> = shell_origins
            .iter()
            .filter(|origin| matches!(origin.host(), Some(url::Host::Ipv4(_) | url::Host::Ipv6(_))))
            .filter_map(|origin| origin.host_str().map(str::to_owned))
            .collect();
        Ok(Self {
            provider,
            shell_origins: origins,
            device_hosts,
            suffixes: unique_suffixes,
            port: primary_origin.port(),
            ports,
            leases: Mutex::new(HashMap::new()),
        })
    }
    fn authority_for(&self, installation: InstallationId, suffix: &str) -> String {
        format!(
            "{installation}.{suffix}{}",
            self.port.map_or(String::new(), |port| format!(":{port}"))
        )
    }
    #[cfg(test)]
    fn authority(&self, installation: InstallationId) -> String {
        self.authority_for(installation, &self.suffixes[0])
    }
    pub fn accepts_host(&self, authority: &str) -> bool {
        if let Some((id, _)) = authority.split_once('.')
            && InstallationId::parse(id).is_ok_and(|installation| {
                self.suffixes
                    .iter()
                    .any(|suffix| self.authority_for(installation, suffix) == authority)
            })
        {
            return true;
        }
        let Some((host, port)) = split_ip_port(authority) else {
            return false;
        };
        self.device_hosts.iter().any(|candidate| candidate == host)
            && self
                .ports
                .as_ref()
                .is_some_and(|ports| ports.installation_for_port(port).is_some())
    }
    fn host_matches_installation(&self, authority: &str, installation: InstallationId) -> bool {
        if self
            .suffixes
            .iter()
            .any(|suffix| self.authority_for(installation, suffix) == authority)
        {
            return true;
        }
        let Some((host, port)) = split_ip_port(authority) else {
            return false;
        };
        self.device_hosts.iter().any(|candidate| candidate == host)
            && self
                .ports
                .as_ref()
                .and_then(|ports| ports.port_for(&installation.to_string()))
                == Some(port)
    }
    fn launch_authority(
        &self,
        installation: InstallationId,
        shell_host: &str,
    ) -> Result<String, AppAccessError> {
        // On the default HTTPS port the Host header has no port, so detect a
        // device IP before stripping an optional port.
        let host = match split_ip_port(shell_host) {
            Some((host, _)) => host,
            None => shell_host,
        };
        if self.device_hosts.iter().any(|candidate| candidate == host) {
            let port = self
                .ports
                .as_ref()
                .and_then(|ports| ports.ensure_port(&installation.to_string()))
                .ok_or(AppAccessError::Unavailable)?;
            return Ok(format!("{host}:{port}"));
        }
        let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
        let suffix = format!("apps.{host}");
        if !self.suffixes.iter().any(|candidate| candidate == &suffix) {
            return Err(AppAccessError::Unavailable);
        }
        Ok(self.authority_for(installation, &suffix))
    }
    pub fn frame_sources(&self) -> Vec<String> {
        let mut sources: Vec<String> = self
            .suffixes
            .iter()
            .map(|suffix| {
                format!(
                    "https://*.{suffix}{}",
                    self.port.map_or(String::new(), |port| format!(":{port}"))
                )
            })
            .collect();
        if let Some(ports) = &self.ports {
            let allocated = ports.all_ports();
            for host in &self.device_hosts {
                for port in &allocated {
                    sources.push(format!("https://{host}:{port}"));
                }
            }
        }
        sources
    }
    pub fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
        let apps = self.provider.catalog(identity)?;
        if apps.len() > 256 {
            return Err(AppAccessError::Unavailable);
        }
        Ok(apps)
    }
    pub fn settings(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
    ) -> Result<AppSettingsInfo, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        let manifest = self.provider.settings(identity, app.installation_id)?;
        Ok(AppSettingsInfo { app, manifest })
    }
    pub fn data(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        path: &str,
    ) -> Result<AppData, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        self.provider.data(identity, app.installation_id, path)
    }
    pub fn runtime(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        action: RuntimeAction,
    ) -> Result<RuntimeState, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| app.id == id && app.installation_id.to_string() == installation)
            .ok_or(AppAccessError::Denied)?;
        match action {
            RuntimeAction::Status => self.provider.runtime_status(identity, app.installation_id),
            RuntimeAction::Start => self.provider.start_runtime(identity, app.installation_id),
            RuntimeAction::Stop => self.provider.stop_runtime(identity, app.installation_id),
        }
    }
    pub fn import_package(
        &self,
        identity: ShellIdentity,
        files: Vec<PackageUploadFile>,
    ) -> Result<ImportedApp, AppAccessError> {
        self.provider.import_package(identity, files)
    }
    pub fn invoke_capability(
        &self,
        identity: ShellIdentity,
        capability: String,
        resource: Option<CapabilityResource>,
    ) -> Result<CapabilityResult, AppAccessError> {
        self.provider
            .invoke_capability(identity, capability, resource)
    }
    pub fn launch(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        previous: Option<&str>,
        shell_host: &str,
    ) -> Result<AppLaunch, AppAccessError> {
        AppId::parse(id).map_err(|_| AppAccessError::Denied)?;
        let app = self
            .catalog(identity)?
            .into_iter()
            .find(|app| {
                app.id == id && app.installation_id.to_string() == installation && app.launchable
            })
            .ok_or(AppAccessError::Denied)?;
        let entry = self.provider.entrypoint(identity, app.installation_id)?;
        PackagePath::parse(&entry).map_err(|_| AppAccessError::Unavailable)?;
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| AppAccessError::Unavailable)?;
        let now = Instant::now();
        leases.retain(|_, lease| lease.expires > now);
        let token = if let Some(previous) = previous {
            leases
                .get(previous)
                .filter(|lease| {
                    lease.identity == identity && lease.installation == app.installation_id
                })
                .ok_or(AppAccessError::Denied)?;
            previous.to_owned()
        } else {
            if leases.len() >= 1024
                || leases
                    .values()
                    .filter(|lease| lease.identity == identity)
                    .count()
                    >= 32
            {
                return Err(AppAccessError::Unavailable);
            }
            let mut random = [0_u8; 32];
            rand::rng().fill_bytes(&mut random);
            random.iter().map(|byte| format!("{byte:02x}")).collect()
        };
        leases.insert(
            token.clone(),
            Lease {
                identity,
                installation: app.installation_id,
                expires: now + Duration::from_secs(90),
            },
        );
        let authority = self.launch_authority(app.installation_id, shell_host)?;
        let frame_url = format!("https://{authority}/launch/{token}/{entry}");
        Ok(AppLaunch {
            app,
            lease: token,
            frame_url,
        })
    }
    pub fn resource(&self, host: &str, path: &str) -> Result<AppAsset, AppAccessError> {
        let rest = path
            .strip_prefix("/launch/")
            .ok_or(AppAccessError::Denied)?;
        let (token, asset) = rest.split_once('/').ok_or(AppAccessError::Denied)?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppAccessError::Denied);
        }
        PackagePath::parse(asset).map_err(|_| AppAccessError::Denied)?;
        let lease = self
            .leases
            .lock()
            .map_err(|_| AppAccessError::Unavailable)?
            .get(token)
            .filter(|lease| lease.expires > Instant::now())
            .cloned()
            .ok_or(AppAccessError::Denied)?;
        if !self.host_matches_installation(host, lease.installation) {
            return Err(AppAccessError::Denied);
        }
        self.provider
            .asset(lease.identity, lease.installation, asset)
    }
    pub fn asset_response(&self, host: &str, asset: AppAsset) -> Response {
        let mut response = Response::new(Body::from(asset.bytes));
        let origin = format!("https://{host}");
        let csp = format!(
            "sandbox allow-scripts; default-src 'none'; script-src {origin} 'unsafe-inline'; style-src {origin} 'unsafe-inline'; img-src {origin} data:; font-src {origin}; connect-src {origin}; frame-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'; frame-ancestors {}",
            self.shell_origins.join(" ")
        );
        let headers = response.headers_mut();
        headers.insert("content-type", HeaderValue::from_static(asset.content_type));
        headers.insert(
            "content-security-policy",
            HeaderValue::from_str(&csp).expect("validated app origin"),
        );
        headers.insert(
            "cache-control",
            HeaderValue::from_static("private, no-store"),
        );
        headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
        headers.insert(
            "x-content-type-options",
            HeaderValue::from_static("nosniff"),
        );
        // Sandboxed ES-module requests have an opaque Origin; never allow credentials.
        headers.insert(
            "access-control-allow-origin",
            HeaderValue::from_static("null"),
        );
        headers.insert(
            "permissions-policy",
            HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
            ),
        );
        response
    }
}
pub fn app_error(error: AppAccessError) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = match error {
        AppAccessError::Denied => StatusCode::NOT_FOUND,
        AppAccessError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

fn split_ip_port(authority: &str) -> Option<(&str, u16)> {
    let (host, port) = authority.rsplit_once(':')?;
    let port = port.parse().ok()?;
    Some((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{SessionId, UserId};
    struct Provider {
        owner: ShellIdentity,
        installation: InstallationId,
    }
    impl AppProvider for Provider {
        fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
            Ok(if identity == self.owner {
                vec![CatalogApp {
                    id: "com.rumahl.test".into(),
                    installation_id: self.installation,
                    title: "Test".into(),
                    version: "1.0.0".into(),
                    launchable: true,
                    capabilities: vec!["com.rumahl.os.window".into()],
                    lifecycle: "on-demand".into(),
                    connectors: vec![],
                }]
            } else {
                vec![]
            })
        }
        fn runtime_status(
            &self,
            _: ShellIdentity,
            _: InstallationId,
        ) -> Result<RuntimeState, AppAccessError> {
            Ok(RuntimeState::Running)
        }
        fn import_package(
            &self,
            _: ShellIdentity,
            _: Vec<PackageUploadFile>,
        ) -> Result<ImportedApp, AppAccessError> {
            Ok(ImportedApp {
                id: "com.rumahl.test".into(),
                installation_id: self.installation,
                title: "Test".into(),
                version: "1.0.0".into(),
            })
        }
        fn entrypoint(
            &self,
            _: ShellIdentity,
            _: InstallationId,
        ) -> Result<String, AppAccessError> {
            Ok("web/index.html".into())
        }
        fn asset(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
            _: &str,
        ) -> Result<AppAsset, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(AppAsset {
                bytes: b"test".to_vec(),
                content_type: "text/html; charset=utf-8",
            })
        }
        fn settings(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
        ) -> Result<Vec<AppSettingInfo>, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(vec![AppSettingInfo {
                key: "server.url".into(),
                title: "Server URL".into(),
                description: Some("Backend endpoint".into()),
                kind: "text".into(),
                required: true,
                options: Vec::new(),
            }])
        }
        fn data(
            &self,
            identity: ShellIdentity,
            installation: InstallationId,
            _: &str,
        ) -> Result<AppData, AppAccessError> {
            if identity != self.owner || installation != self.installation {
                return Err(AppAccessError::Denied);
            }
            Ok(AppData::Directory(vec![AppDataEntry {
                name: "notes.db".into(),
                directory: false,
                size: 12,
            }]))
        }
        fn invoke_capability(
            &self,
            identity: ShellIdentity,
            capability: String,
            _: Option<CapabilityResource>,
        ) -> Result<CapabilityResult, AppAccessError> {
            if identity != self.owner {
                return Err(AppAccessError::Denied);
            }
            Ok(match capability.as_str() {
                "com.rumahl.test.run" => CapabilityResult {
                    outcome: CapabilityOutcome::Invoked,
                    result: Some("{\"ran\":true}".into()),
                    browser: None,
                },
                "com.rumahl.test.browser" => CapabilityResult {
                    outcome: CapabilityOutcome::Invoked,
                    result: None,
                    browser: Some(BrowserDelivery {
                        app_id: "com.rumahl.test".into(),
                        installation_id: self.installation.to_string(),
                    }),
                },
                _ => CapabilityResult {
                    outcome: CapabilityOutcome::Denied,
                    result: None,
                    browser: None,
                },
            })
        }
    }
    #[test]
    fn import_delegates_to_the_provider() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            &[Url::parse("https://localhost:8443").unwrap()],
            &["apps.localhost".to_owned()],
            None,
        )
        .unwrap();
        let imported = access
            .import_package(
                owner,
                vec![PackageUploadFile {
                    path: "package.json".into(),
                    bytes: b"{}".to_vec(),
                }],
            )
            .unwrap();
        assert_eq!(imported.installation_id, installation);
        assert_eq!(imported.id, "com.rumahl.test");
    }

    #[test]
    fn invoke_capability_delegates_to_the_provider() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            &[Url::parse("https://localhost:8443").unwrap()],
            &["apps.localhost".to_owned()],
            None,
        )
        .unwrap();
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.test.run".into(), None)
                .unwrap(),
            CapabilityResult {
                outcome: CapabilityOutcome::Invoked,
                result: Some("{\"ran\":true}".into()),
                browser: None
            }
        );
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.unknown".into(), None)
                .unwrap(),
            CapabilityResult {
                outcome: CapabilityOutcome::Denied,
                result: None,
                browser: None
            }
        );
        assert_eq!(
            access
                .invoke_capability(owner, "com.rumahl.test.browser".into(), None)
                .unwrap()
                .browser,
            Some(BrowserDelivery {
                app_id: "com.rumahl.test".into(),
                installation_id: installation.to_string()
            })
        );
    }

    #[test]
    fn runtime_control_reports_state_and_denies_other_installations() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            &[Url::parse("https://localhost:8443").unwrap()],
            &["apps.localhost".to_owned()],
            None,
        )
        .unwrap();
        assert_eq!(
            access
                .runtime(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    RuntimeAction::Status
                )
                .unwrap(),
            RuntimeState::Running
        );
        assert_eq!(
            access
                .runtime(
                    owner,
                    "com.rumahl.test",
                    &InstallationId::new().to_string(),
                    RuntimeAction::Status
                )
                .unwrap_err(),
            AppAccessError::Denied
        );
    }

    #[test]
    fn leases_bind_host_installation_session_and_expire() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            &[Url::parse("https://localhost:8443").unwrap()],
            &["apps.localhost".to_owned()],
            None,
        )
        .unwrap();
        let launch = access
            .launch(
                owner,
                "com.rumahl.test",
                &installation.to_string(),
                None,
                "localhost:8443",
            )
            .unwrap();
        let url = Url::parse(&launch.frame_url).unwrap();
        let host = access.authority(installation);
        assert!(access.accepts_host(&host));
        assert!(!access.accepts_host("localhost:8443"));
        assert!(!access.accepts_host(&format!("{host}.evil.test")));
        assert!(access.resource(&host, url.path()).is_ok());
        assert!(access.resource("localhost:8443", url.path()).is_err());
        assert!(
            access
                .resource(&access.authority(InstallationId::new()), url.path())
                .is_err()
        );
        assert!(
            access
                .resource(&host, &format!("/launch/{}/../secret.json", launch.lease))
                .is_err()
        );
        let other = ShellIdentity {
            session_id: SessionId::new(),
            ..owner
        };
        assert!(
            access
                .launch(
                    other,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease),
                    "localhost:8443",
                )
                .is_err()
        );
        assert!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &InstallationId::new().to_string(),
                    Some(&launch.lease),
                    "localhost:8443",
                )
                .is_err()
        );
        assert_eq!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease),
                    "localhost:8443",
                )
                .unwrap()
                .frame_url,
            launch.frame_url
        );
        let response = access.asset_response(&host, access.resource(&host, url.path()).unwrap());
        assert_eq!(response.headers()["access-control-allow-origin"], "null");
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .starts_with("sandbox allow-scripts;")
        );
        access
            .leases
            .lock()
            .unwrap()
            .get_mut(&launch.lease)
            .unwrap()
            .expires = Instant::now() - Duration::from_secs(1);
        assert!(access.resource(&host, url.path()).is_err());
        assert!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease),
                    "localhost:8443",
                )
                .is_err()
        );
    }

    #[test]
    fn settings_are_scoped_to_the_installed_app() {
        let owner = ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        };
        let installation = InstallationId::new();
        let access = AppAccess::new(
            Arc::new(Provider {
                owner,
                installation,
            }),
            &Url::parse("https://localhost:8443").unwrap(),
            &[Url::parse("https://localhost:8443").unwrap()],
            &["apps.localhost".to_owned()],
            None,
        )
        .unwrap();

        let settings = access
            .settings(owner, "com.rumahl.test", &installation.to_string())
            .unwrap();
        assert_eq!(settings.manifest.len(), 1);
        assert_eq!(settings.manifest[0].key, "server.url");
        assert_eq!(settings.manifest[0].kind, "text");
        assert!(settings.manifest[0].required);

        // Unknown installation or unauthenticated identity is denied.
        assert!(
            access
                .settings(owner, "com.rumahl.test", &InstallationId::new().to_string())
                .is_err()
        );
        assert!(
            access
                .settings(
                    ShellIdentity {
                        session_id: SessionId::new(),
                        ..owner
                    },
                    "com.rumahl.test",
                    &installation.to_string()
                )
                .is_err()
        );
    }
}
