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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAccessError {
    Denied,
    Unavailable,
}
pub struct AppAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
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
}
#[derive(Clone)]
struct Lease {
    identity: ShellIdentity,
    installation: InstallationId,
    expires: Instant,
}
pub struct AppAccess {
    provider: Arc<dyn AppProvider>,
    shell_origin: String,
    suffix: String,
    port: Option<u16>,
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
        shell_origin: &Url,
        suffix: &str,
    ) -> Result<Self, AppAccessError> {
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
        if !valid || shell_origin.host_str() == Some(suffix) || shell_origin.scheme() != "https" {
            return Err(AppAccessError::Unavailable);
        }
        Ok(Self {
            provider,
            shell_origin: shell_origin.origin().ascii_serialization(),
            suffix: suffix.into(),
            port: shell_origin.port(),
            leases: Mutex::new(HashMap::new()),
        })
    }
    fn authority(&self, installation: InstallationId) -> String {
        format!(
            "{installation}.{}{}",
            self.suffix,
            self.port.map_or(String::new(), |port| format!(":{port}"))
        )
    }
    pub fn accepts_host(&self, authority: &str) -> bool {
        let Some((id, _)) = authority.split_once('.') else {
            return false;
        };
        InstallationId::parse(id)
            .is_ok_and(|installation| self.authority(installation) == authority)
    }
    pub fn frame_source(&self) -> String {
        format!(
            "https://*.{}{}",
            self.suffix,
            self.port.map_or(String::new(), |port| format!(":{port}"))
        )
    }
    pub fn catalog(&self, identity: ShellIdentity) -> Result<Vec<CatalogApp>, AppAccessError> {
        let apps = self.provider.catalog(identity)?;
        if apps.len() > 256 {
            return Err(AppAccessError::Unavailable);
        }
        Ok(apps)
    }
    pub fn launch(
        &self,
        identity: ShellIdentity,
        id: &str,
        installation: &str,
        previous: Option<&str>,
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
        let frame_url = format!(
            "https://{}/launch/{token}/{entry}",
            self.authority(app.installation_id)
        );
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
        if host != self.authority(lease.installation) {
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
            self.shell_origin
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
                }]
            } else {
                vec![]
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
            "apps.localhost",
        )
        .unwrap();
        let launch = access
            .launch(owner, "com.rumahl.test", &installation.to_string(), None)
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
                    Some(&launch.lease)
                )
                .is_err()
        );
        assert!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &InstallationId::new().to_string(),
                    Some(&launch.lease)
                )
                .is_err()
        );
        assert_eq!(
            access
                .launch(
                    owner,
                    "com.rumahl.test",
                    &installation.to_string(),
                    Some(&launch.lease)
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
                    Some(&launch.lease)
                )
                .is_err()
        );
    }
}
