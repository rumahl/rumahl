//! Website and GitHub app-store sources.
//!
//! Sources are persisted as a small owner-only JSON document. Network access is
//! deliberately server-side: the shell sends only a source kind, a location and
//! the app the user selected. Every downloaded package still passes the normal
//! signed-package verifier before anything is installed or published.
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rumahl_app_packages::{MAX_MANIFEST_BYTES, PackageManifest};
use rumahl_core::{AppId, AppVersion};
use rumahl_platform_web::{
    ImportedApp, ShellIdentity, StoreApp, StoreError, StoreListing, StoreProvider, StoreSource,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

const SOURCES_FILE: &str = "app-store-sources.json";
const SOURCES_VERSION: u32 = 1;
const MAX_SOURCES: usize = 64;
const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
const MAX_APPS_PER_SOURCE: usize = 4096;
const MAX_SOURCE_LOCATION: usize = 2048;
const MAX_TITLE_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 2000;
const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_GITHUB_RAW_BASE: &str = "https://raw.githubusercontent.com";
const DEFAULT_GITHUB_INDEX: &str = "store.json";
const DEFAULT_GITHUB_REF: &str = "main";

/// A website or GitHub source after parsing and normalization.
#[derive(Debug, Clone)]
enum SourceLocation {
    Website { index: Url },
    Github(GithubRepo),
}

#[derive(Debug, Clone)]
struct GithubRepo {
    owner: String,
    repo: String,
    reference: String,
    /// Path to the store index inside the repository.
    index: String,
}

#[derive(Debug, Clone)]
struct ResolvedApp {
    app: StoreApp,
    package_url: Url,
}

#[derive(Debug, Clone)]
struct ResolvedIndex {
    title: String,
    apps: Vec<ResolvedApp>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SourcesDocument {
    format_version: u32,
    sources: Vec<StoredSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSource {
    id: String,
    kind: String,
    location: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawIndex {
    format_version: u32,
    #[serde(default)]
    name: Option<String>,
    apps: Vec<RawIndexApp>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawIndexApp {
    id: String,
    title: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    package_url: String,
}

#[derive(Debug)]
enum FetchError {
    NotFound,
    Unavailable,
    TooLarge,
}

/// Persisted app-store sources plus the HTTP client used to resolve them.
pub struct AppStore {
    state_dir: PathBuf,
    sources_path: PathBuf,
    github_raw_base: String,
    default_client: reqwest::blocking::Client,
    /// Built lazily so a local test CA generated after startup is still trusted.
    extra_client: Mutex<Option<reqwest::blocking::Client>>,
    ca_file: Option<PathBuf>,
    /// Serializes source-file mutations without blocking reads.
    write_lock: Mutex<()>,
}

impl AppStore {
    pub fn open(state_dir: &Path) -> Result<Self, io::Error> {
        if !state_dir.is_absolute() {
            return Err(io::Error::other(
                "app store state directory must be absolute",
            ));
        }
        let github_raw_base = std::env::var("RUMAHL_GITHUB_RAW_BASE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GITHUB_RAW_BASE.to_owned())
            .trim_end_matches('/')
            .to_owned();
        Url::parse(&github_raw_base)
            .ok()
            .filter(secure_url)
            .ok_or_else(|| io::Error::other("GitHub raw base must use HTTPS"))?;
        let default_client = create_client(None)?;
        let ca_file = std::env::var_os("RUMAHL_APP_STORE_CA_FILE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(Self {
            state_dir: state_dir.to_owned(),
            sources_path: state_dir.join(SOURCES_FILE),
            github_raw_base,
            default_client,
            extra_client: Mutex::new(None),
            ca_file,
            write_lock: Mutex::new(()),
        })
    }

    /// Returns the HTTPS client, adding the configured local CA once available.
    fn client(&self) -> reqwest::blocking::Client {
        if let Some(path) = &self.ca_file
            && let Ok(mut extra) = self.extra_client.lock()
        {
            if extra.is_none()
                && let Ok(bytes) = fs::read(path)
                && let Ok(certificate) = reqwest::Certificate::from_pem(&bytes)
                && let Ok(client) = create_client(Some(certificate))
            {
                *extra = Some(client);
            }
            if let Some(client) = extra.as_ref() {
                return client.clone();
            }
        }
        self.default_client.clone()
    }

    fn load_sources(&self) -> Result<SourcesDocument, StoreError> {
        let bytes = match fs::read(&self.sources_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SourcesDocument {
                    format_version: SOURCES_VERSION,
                    sources: Vec::new(),
                });
            }
            Err(_) => return Err(StoreError::Unavailable),
        };
        if bytes.len() > 256 * 1024 {
            return Err(StoreError::Unavailable);
        }
        let document: SourcesDocument =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::Unavailable)?;
        if document.format_version != SOURCES_VERSION || document.sources.len() > MAX_SOURCES {
            return Err(StoreError::Unavailable);
        }
        let mut seen = std::collections::HashSet::new();
        for source in &document.sources {
            if !valid_source_id(&source.id)
                || !matches!(source.kind.as_str(), "website" | "github")
                || source.location.is_empty()
                || source.location.len() > MAX_SOURCE_LOCATION
                || !seen.insert(source.id.as_str())
            {
                return Err(StoreError::Unavailable);
            }
        }
        Ok(document)
    }

    fn save_sources(&self, document: &SourcesDocument) -> Result<(), StoreError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| StoreError::Unavailable)?;
        let temporary = self.sources_path.with_file_name(format!(
            ".{SOURCES_FILE}.{}-{}",
            std::process::id(),
            now_nanos()
        ));
        let result = (|| -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec_pretty(document).map_err(io::Error::other)?)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.sources_path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(StoreError::Unavailable);
        }
        Ok(())
    }

    fn source_for(&self, id: &str) -> Result<StoredSource, StoreError> {
        self.load_sources()?
            .sources
            .into_iter()
            .find(|source| source.id == id)
            .ok_or(StoreError::InvalidRequest)
    }

    fn resolve_location(kind: &str, location: &str) -> Result<SourceLocation, StoreError> {
        match kind {
            "website" => website_index(location).map(|index| SourceLocation::Website { index }),
            "github" => parse_github(location).map(SourceLocation::Github),
            _ => Err(StoreError::InvalidRequest),
        }
    }

    fn canonical_location(location: &SourceLocation) -> String {
        match location {
            SourceLocation::Website { index } => index.as_str().to_owned(),
            SourceLocation::Github(repo) => format!(
                "github:{}/{}@{}/{}",
                repo.owner, repo.repo, repo.reference, repo.index
            ),
        }
    }

    fn fetch_index(&self, location: &SourceLocation) -> Result<ResolvedIndex, StoreError> {
        match location {
            SourceLocation::Website { index } => {
                let bytes = self
                    .fetch(index, MAX_INDEX_BYTES)
                    .map_err(map_fetch_error)?;
                self.parse_index(&bytes, index)
            }
            SourceLocation::Github(repo) => {
                let index_url = self.github_url(repo, &repo.index)?;
                match self.fetch(&index_url, MAX_INDEX_BYTES) {
                    Ok(bytes) => self.parse_index(&bytes, &index_url),
                    Err(FetchError::NotFound) => self.github_single_app(repo),
                    Err(error) => Err(map_fetch_error(error)),
                }
            }
        }
    }

    /// Accepts a repository whose root itself is one signed package and has no
    /// store index. This keeps simple one-app repositories usable as a source.
    fn github_single_app(&self, repo: &GithubRepo) -> Result<ResolvedIndex, StoreError> {
        let package_url = self.github_url(repo, "")?;
        let manifest_url = package_url
            .join("package.json")
            .map_err(|_| StoreError::InvalidRequest)?;
        let bytes = self
            .fetch(&manifest_url, MAX_MANIFEST_BYTES)
            .map_err(map_fetch_error)?;
        let manifest =
            PackageManifest::from_bytes(&bytes).map_err(|_| StoreError::InvalidRequest)?;
        let app = StoreApp {
            source_id: String::new(),
            id: manifest.app().app_id().as_str().to_owned(),
            title: manifest.app().display_name().to_owned(),
            version: manifest.app().version().to_string(),
            description: None,
            publisher: Some(manifest.publisher_id().as_str().to_owned()),
            icon: None,
        };
        Ok(ResolvedIndex {
            title: format!("{}/{}", repo.owner, repo.repo),
            apps: vec![ResolvedApp { app, package_url }],
        })
    }

    fn parse_index(&self, bytes: &[u8], index_url: &Url) -> Result<ResolvedIndex, StoreError> {
        let raw: RawIndex =
            serde_json::from_slice(bytes).map_err(|_| StoreError::InvalidRequest)?;
        if raw.format_version != 1 || raw.apps.len() > MAX_APPS_PER_SOURCE {
            return Err(StoreError::InvalidRequest);
        }
        let base = directory_url(index_url);
        let title = raw
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.chars().count() <= MAX_TITLE_CHARS)
            .map(str::to_owned)
            .or_else(|| index_url.host_str().map(str::to_owned))
            .unwrap_or_else(|| "App source".to_owned());
        let mut seen = std::collections::HashSet::new();
        let mut apps = Vec::with_capacity(raw.apps.len());
        for app in raw.apps {
            let Ok(id) = AppId::parse(app.id.clone()) else {
                continue;
            };
            if !seen.insert(id.as_str().to_owned()) {
                continue;
            }
            let Ok(version) = AppVersion::parse(&app.version) else {
                continue;
            };
            let title = app.title.trim();
            if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
                continue;
            }
            let description = app
                .description
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty() && text.chars().count() <= MAX_DESCRIPTION_CHARS)
                .map(str::to_owned);
            let publisher = app
                .publisher
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty() && text.len() <= 128)
                .map(str::to_owned);
            let mut package_url = match base.join(&app.package_url) {
                Ok(url) => url,
                Err(_) => continue,
            };
            if !secure_url(&package_url)
                || package_url.username() != ""
                || package_url.password().is_some()
                || package_url.fragment().is_some()
                || package_url.as_str().len() > MAX_SOURCE_LOCATION
            {
                continue;
            }
            package_url.set_query(None);
            if !package_url.path().ends_with('/') {
                let path = format!("{}/", package_url.path());
                package_url.set_path(&path);
            }
            let icon = app.icon.and_then(|icon| {
                let icon = base.join(&icon).ok()?;
                secure_url(&icon).then(|| icon.as_str().to_owned())
            });
            apps.push(ResolvedApp {
                app: StoreApp {
                    source_id: String::new(),
                    id: id.as_str().to_owned(),
                    title: title.to_owned(),
                    version: version.to_string(),
                    description,
                    publisher,
                    icon,
                },
                package_url,
            });
        }
        Ok(ResolvedIndex { title, apps })
    }

    fn github_url(&self, repo: &GithubRepo, path: &str) -> Result<Url, StoreError> {
        let base = format!(
            "{}/{}/{}/{}",
            self.github_raw_base, repo.owner, repo.repo, repo.reference
        );
        let value = if path.is_empty() {
            format!("{base}/")
        } else {
            format!("{base}/{path}")
        };
        let url = Url::parse(&value).map_err(|_| StoreError::InvalidRequest)?;
        secure_url(&url)
            .then_some(url)
            .ok_or(StoreError::InvalidRequest)
    }

    fn fetch(&self, url: &Url, maximum: usize) -> Result<Vec<u8>, FetchError> {
        let response = self.client().get(url.as_str()).send().map_err(|_| {
            if url.scheme() == "http" || url.scheme() == "https" {
                FetchError::Unavailable
            } else {
                FetchError::NotFound
            }
        })?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(FetchError::NotFound);
        }
        if !response.status().is_success() {
            return Err(FetchError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > maximum as u64)
        {
            return Err(FetchError::TooLarge);
        }
        let mut bytes = Vec::new();
        response
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FetchError::Unavailable)?;
        if bytes.len() > maximum {
            return Err(FetchError::TooLarge);
        }
        Ok(bytes)
    }

    fn download_package(&self, app: &ResolvedApp) -> Result<DownloadedPackage, StoreError> {
        let manifest_url = app
            .package_url
            .join("package.json")
            .map_err(|_| StoreError::InvalidRequest)?;
        let manifest_bytes = self
            .fetch(&manifest_url, MAX_MANIFEST_BYTES)
            .map_err(map_fetch_error)?;
        let manifest =
            PackageManifest::from_bytes(&manifest_bytes).map_err(|_| StoreError::InvalidRequest)?;
        if manifest.app().app_id().as_str() != app.app.id {
            return Err(StoreError::InvalidRequest);
        }
        let signature = self
            .fetch(
                &app.package_url
                    .join("package.sig.json")
                    .map_err(|_| StoreError::InvalidRequest)?,
                4 * 1024,
            )
            .map_err(map_fetch_error)?;

        let staging = self.state_dir.join("store-staging").join(format!(
            "{}-{}",
            std::process::id(),
            now_nanos()
        ));
        let result = (|| -> Result<DownloadedPackage, StoreError> {
            fs::create_dir_all(&staging).map_err(|_| StoreError::Unavailable)?;
            fs::set_permissions(
                &staging,
                std::os::unix::fs::PermissionsExt::from_mode(0o700),
            )
            .map_err(|_| StoreError::Unavailable)?;
            write_file(&staging.join("package.json"), &manifest_bytes)?;
            write_file(&staging.join("package.sig.json"), &signature)?;

            let mut total = 0_u64;
            for file in manifest.files() {
                total = total.saturating_add(file.size());
                if total > MAX_PACKAGE_BYTES {
                    return Err(StoreError::InvalidRequest);
                }
                let url = app
                    .package_url
                    .join(file.path().as_str())
                    .map_err(|_| StoreError::InvalidRequest)?;
                let bytes = self
                    .fetch(
                        &url,
                        usize::try_from(file.size()).map_err(|_| StoreError::InvalidRequest)?,
                    )
                    .map_err(map_fetch_error)?;
                if bytes.len() as u64 != file.size() {
                    return Err(StoreError::InvalidRequest);
                }
                let target = staging.join(file.path().as_str());
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|_| StoreError::Unavailable)?;
                    fs::set_permissions(
                        parent,
                        std::os::unix::fs::PermissionsExt::from_mode(0o700),
                    )
                    .map_err(|_| StoreError::Unavailable)?;
                }
                write_file(&target, &bytes)?;
            }
            Ok(DownloadedPackage {
                path: staging.clone(),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    fn install_package(&self, app: &ResolvedApp) -> Result<ImportedApp, StoreError> {
        let package = self.download_package(app)?;
        let result = crate::install::install_package_dir(&self.state_dir, &package.path);
        let _ = fs::remove_dir_all(&package.path);
        let info = result.map_err(|_| StoreError::Unavailable)?;
        Ok(ImportedApp {
            id: info.app_id,
            installation_id: info.installation_id,
            title: info.title,
            version: info.version,
        })
    }
}

struct DownloadedPackage {
    path: PathBuf,
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| StoreError::Unavailable)?;
    file.write_all(bytes).map_err(|_| StoreError::Unavailable)?;
    file.sync_all().map_err(|_| StoreError::Unavailable)
}

impl StoreProvider for AppStore {
    fn list(&self, _identity: ShellIdentity) -> Result<StoreListing, StoreError> {
        let document = self.load_sources()?;
        let mut sources = Vec::with_capacity(document.sources.len());
        let mut apps = Vec::new();
        for stored in document.sources {
            let location = AppStore::resolve_location(&stored.kind, &stored.location);
            match location.and_then(|location| self.fetch_index(&location)) {
                Ok(index) => {
                    sources.push(StoreSource {
                        id: stored.id.clone(),
                        kind: stored.kind,
                        location: stored.location,
                        title: index.title.clone(),
                        status: "ready".to_owned(),
                        error: None,
                    });
                    apps.extend(index.apps.into_iter().map(|mut app| {
                        app.app.source_id = stored.id.clone();
                        app.app
                    }));
                }
                Err(_) => {
                    let title = fallback_source_title(&stored.kind, &stored.location);
                    sources.push(StoreSource {
                        id: stored.id,
                        kind: stored.kind,
                        location: stored.location,
                        title,
                        status: "unavailable".to_owned(),
                        error: None,
                    });
                }
            }
        }
        Ok(StoreListing { sources, apps })
    }

    fn add_source(
        &self,
        _identity: ShellIdentity,
        kind: &str,
        location: &str,
    ) -> Result<StoreSource, StoreError> {
        if !matches!(kind, "website" | "github")
            || location.is_empty()
            || location.len() > MAX_SOURCE_LOCATION
        {
            return Err(StoreError::InvalidRequest);
        }
        let location = AppStore::resolve_location(kind, location.trim())?;
        let index = self.fetch_index(&location)?;
        let canonical = AppStore::canonical_location(&location);
        let id = source_id(kind, &canonical);
        let mut document = self.load_sources()?;
        if document.sources.len() >= MAX_SOURCES && !document.sources.iter().any(|s| s.id == id) {
            return Err(StoreError::InvalidRequest);
        }
        if !document.sources.iter().any(|source| source.id == id) {
            document.sources.push(StoredSource {
                id: id.clone(),
                kind: kind.to_owned(),
                location: canonical.clone(),
            });
            self.save_sources(&document)?;
        }
        Ok(StoreSource {
            id,
            kind: kind.to_owned(),
            location: canonical,
            title: index.title,
            status: "ready".to_owned(),
            error: None,
        })
    }

    fn remove_source(&self, _identity: ShellIdentity, id: &str) -> Result<(), StoreError> {
        if !valid_source_id(id) {
            return Err(StoreError::InvalidRequest);
        }
        let mut document = self.load_sources()?;
        let before = document.sources.len();
        document.sources.retain(|source| source.id != id);
        if document.sources.len() != before {
            self.save_sources(&document)?;
        }
        Ok(())
    }

    fn install(
        &self,
        _identity: ShellIdentity,
        source_id: &str,
        app_id: &str,
    ) -> Result<ImportedApp, StoreError> {
        if !valid_source_id(source_id) {
            return Err(StoreError::InvalidRequest);
        }
        let source = self.source_for(source_id)?;
        let location = AppStore::resolve_location(&source.kind, &source.location)?;
        let index = self.fetch_index(&location)?;
        let app = index
            .apps
            .into_iter()
            .find(|app| app.app.id == app_id)
            .ok_or(StoreError::InvalidRequest)?;
        self.install_package(&app)
    }
}

#[allow(dead_code)]
fn fetch_error_message(error: &FetchError) -> &'static str {
    match error {
        FetchError::NotFound => "source document not found",
        FetchError::Unavailable => "source unavailable",
        FetchError::TooLarge => "source document too large",
    }
}

fn map_fetch_error(error: FetchError) -> StoreError {
    match error {
        FetchError::NotFound | FetchError::TooLarge => StoreError::InvalidRequest,
        FetchError::Unavailable => StoreError::Unavailable,
    }
}

fn fallback_source_title(kind: &str, location: &str) -> String {
    match AppStore::resolve_location(kind, location) {
        Ok(SourceLocation::Website { index }) => index.host_str().map(str::to_owned),
        Ok(SourceLocation::Github(repo)) => Some(format!("{}/{}", repo.owner, repo.repo)),
        Err(_) => None,
    }
    .unwrap_or_else(|| location.to_owned())
}

/// Builds the blocking HTTPS client on a plain thread.
///
/// `reqwest::blocking` owns a private runtime and panics when it is created
/// while a Tokio runtime is entered; the service opens the store from async
/// code, so the dependency is constructed outside that context.
fn create_client(
    root_certificate: Option<reqwest::Certificate>,
) -> Result<reqwest::blocking::Client, io::Error> {
    std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let builder = store_client_builder();
                let builder = match root_certificate {
                    Some(certificate) => builder.add_root_certificate(certificate),
                    None => builder,
                };
                builder.build()
            })
            .join()
            .map_err(|_| io::Error::other("app store HTTPS client could not be created"))?
            .map_err(|_| io::Error::other("app store HTTPS client could not be created"))
    })
}

fn store_client_builder() -> reqwest::blocking::ClientBuilder {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 || !secure_url(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .user_agent("rumahl-app-store/1")
}

/// Public sources must use HTTPS. Plain HTTP is accepted only for loopback test
/// servers, never for a reachable network host.
fn secure_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => url.host_str().is_some_and(loopback_host),
        _ => false,
    }
}

fn loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn valid_source_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn source_id(kind: &str, location: &str) -> String {
    let digest = Sha256::digest(format!("rumahl-app-store-v1\0{kind}\0{location}").as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn directory_url(url: &Url) -> Url {
    let mut base = url.clone();
    base.set_query(None);
    base.set_fragment(None);
    let path = base.path().to_owned();
    let directory = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    base.set_path(&format!("{directory}/"));
    base
}

fn website_index(location: &str) -> Result<Url, StoreError> {
    let mut url = Url::parse(location).map_err(|_| StoreError::InvalidRequest)?;
    if !secure_url(&url)
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
        || url.as_str().len() > MAX_SOURCE_LOCATION
    {
        return Err(StoreError::InvalidRequest);
    }
    url.set_query(None);
    let path = url.path().to_owned();
    let last = path.rsplit('/').next().unwrap_or("");
    if !last.ends_with(".json") {
        let path = if path.ends_with('/') {
            format!("{path}{DEFAULT_GITHUB_INDEX}")
        } else {
            format!("{path}/{DEFAULT_GITHUB_INDEX}")
        };
        url.set_path(&path);
    }
    Ok(url)
}

fn parse_github(location: &str) -> Result<GithubRepo, StoreError> {
    let location = location.trim();
    if location.is_empty() || location.len() > MAX_SOURCE_LOCATION {
        return Err(StoreError::InvalidRequest);
    }
    if let Some(rest) = location.strip_prefix("github:") {
        let (repository, rest) = rest.split_once('@').ok_or(StoreError::InvalidRequest)?;
        let (owner, repo) = split_repository(repository)?;
        let (reference, index) = rest.split_once('/').ok_or(StoreError::InvalidRequest)?;
        return github_repo(owner, repo, reference, index);
    }
    if let Ok(url) = Url::parse(location) {
        let host = url.host_str().ok_or(StoreError::InvalidRequest)?;
        if url.username() != "" || url.password().is_some() || url.fragment().is_some() {
            return Err(StoreError::InvalidRequest);
        }
        let segments: Vec<&str> = url
            .path_segments()
            .map(|parts| parts.filter(|part| !part.is_empty()).collect())
            .unwrap_or_default();
        match host {
            "github.com" if segments.len() >= 2 => {
                let repository = format!("{}/{}", segments[0], segments[1]);
                let (owner, repo) = split_repository(&repository)?;
                if segments.len() == 2 {
                    return github_repo(owner, repo, DEFAULT_GITHUB_REF, DEFAULT_GITHUB_INDEX);
                }
                match (segments[2], segments.get(3)) {
                    ("tree", Some(reference)) if segments.len() >= 4 => {
                        let path = segments[4..].join("/");
                        return github_repo(owner, repo, reference, &path);
                    }
                    ("blob", Some(reference)) if segments.len() >= 4 => {
                        let path = segments[4..].join("/");
                        return github_repo(owner, repo, reference, &path);
                    }
                    _ => return Err(StoreError::InvalidRequest),
                }
            }
            "raw.githubusercontent.com" if segments.len() >= 4 => {
                let repository = format!("{}/{}", segments[0], segments[1]);
                let (owner, repo) = split_repository(&repository)?;
                let path = segments[3..].join("/");
                return github_repo(owner, repo, segments[2], &path);
            }
            _ => {}
        }
        return Err(StoreError::InvalidRequest);
    }
    let (owner, repo) = split_repository(location)?;
    github_repo(owner, repo, DEFAULT_GITHUB_REF, DEFAULT_GITHUB_INDEX)
}

fn split_repository(value: &str) -> Result<(&str, &str), StoreError> {
    let (owner, repo) = value.split_once('/').ok_or(StoreError::InvalidRequest)?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if !safe_component(owner) || !safe_component(repo) {
        return Err(StoreError::InvalidRequest);
    }
    Ok((owner, repo))
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn github_repo(
    owner: &str,
    repo: &str,
    reference: &str,
    index: &str,
) -> Result<GithubRepo, StoreError> {
    let reference = reference.trim();
    if !safe_component(reference) {
        return Err(StoreError::InvalidRequest);
    }
    let mut index = index.trim().trim_start_matches('/').to_owned();
    if index.is_empty() {
        index = DEFAULT_GITHUB_INDEX.to_owned();
    }
    if index.ends_with('/') {
        index.push_str(DEFAULT_GITHUB_INDEX);
    } else if !index.rsplit('/').next().unwrap_or("").contains('.') {
        index.push('/');
        index.push_str(DEFAULT_GITHUB_INDEX);
    }
    if index.len() > 512
        || index
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || !index
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return Err(StoreError::InvalidRequest);
    }
    Ok(GithubRepo {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        reference: reference.to_owned(),
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{SessionId, UserId};
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    fn identity() -> ShellIdentity {
        ShellIdentity {
            user_id: UserId::new(),
            session_id: SessionId::new(),
        }
    }

    fn serve_index(index: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().take(4) {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    index.len(),
                    index
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn normalizes_website_indexes_and_github_locations() {
        assert_eq!(
            website_index("https://apps.example.com").unwrap().as_str(),
            "https://apps.example.com/store.json"
        );
        assert_eq!(
            website_index("https://apps.example.com/catalog/")
                .unwrap()
                .as_str(),
            "https://apps.example.com/catalog/store.json"
        );
        assert_eq!(
            website_index("https://apps.example.com/catalog/apps.json")
                .unwrap()
                .as_str(),
            "https://apps.example.com/catalog/apps.json"
        );
        let repo = parse_github("rumahl/apps").unwrap();
        assert_eq!(repo.owner, "rumahl");
        assert_eq!(repo.repo, "apps");
        assert_eq!(repo.reference, "main");
        assert_eq!(repo.index, "store.json");
        let repo = parse_github("https://github.com/rumahl/apps/tree/stable/store").unwrap();
        assert_eq!(repo.reference, "stable");
        assert_eq!(repo.index, "store/store.json");
        let repo = parse_github("github:rumahl/apps@next/index.json").unwrap();
        assert_eq!(repo.reference, "next");
        assert_eq!(repo.index, "index.json");
        assert!(parse_github("https://example.com/rumahl/apps").is_err());
        assert!(parse_github("../escape/repo").is_err());
    }

    fn serve_directory(root: PathBuf) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().take(16) {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                if reader.read_line(&mut request).unwrap_or(0) == 0 {
                    continue;
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let path = path
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let file = root.join(path);
                let (status, bytes, content_type) = if file.is_file() {
                    (
                        "200 OK",
                        fs::read(&file).unwrap_or_default(),
                        if file.extension().and_then(|value| value.to_str()) == Some("json") {
                            "application/json"
                        } else {
                            "text/html"
                        },
                    )
                } else {
                    ("404 Not Found", Vec::new(), "text/plain")
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn downloads_and_verifies_a_signed_package() {
        use rumahl_app_packages::{
            GeneratedKey, PackageManifest, PackageVerifier, TrustStore, trust_store_json,
        };
        use serde_json::json;

        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("site/packages/notes");
        fs::create_dir_all(package.join("frontend")).unwrap();
        fs::write(package.join("frontend/index.html"), b"<h1>Notes</h1>").unwrap();
        let generated = GeneratedKey::generate("test-key").unwrap();
        let template = json!({
            "formatVersion": 1,
            "publisherId": "com.rumahl.examples",
            "app": {
                "appId": "com.rumahl.examples.notes",
                "version": "1.0.0",
                "displayName": "Notes",
                "runtime": {
                    "kind": "web",
                    "entrypoints": [
                        { "id": "main", "kind": "web-asset", "path": "frontend/index.html" },
                    ],
                },
            },
        });
        let manifest =
            PackageManifest::from_bytes(&serde_json::to_vec(&template).unwrap()).unwrap();
        generated
            .key_store()
            .signer()
            .unwrap()
            .sign(&package, &manifest)
            .unwrap();
        let trust_store = serde_json::to_vec(&trust_store_json(
            "test-key",
            "com.rumahl.examples",
            &generated.public_key(),
        ))
        .unwrap();
        fs::write(root.path().join("trust.json"), &trust_store).unwrap();
        fs::write(
            root.path().join("site/store.json"),
            serde_json::to_vec(&json!({
                "formatVersion": 1,
                "name": "Test store",
                "apps": [{
                    "id": "com.rumahl.examples.notes",
                    "title": "Notes",
                    "version": "1.0.0",
                    "packageUrl": "packages/notes",
                }],
            }))
            .unwrap(),
        )
        .unwrap();
        let server = serve_directory(root.path().to_owned());
        let state = tempfile::tempdir().unwrap();
        let store = AppStore::open(state.path()).unwrap();
        let source = store
            .add_source(identity(), "website", &format!("{server}/site/store.json"))
            .unwrap();
        let location = AppStore::resolve_location("website", &source.location).unwrap();
        let index = store.fetch_index(&location).unwrap();
        let app = index.apps.first().unwrap();
        let downloaded = store.download_package(app).unwrap();
        let verifier = PackageVerifier::new(TrustStore::from_bytes(&trust_store).unwrap());
        let verified = verifier.verify(&downloaded.path).unwrap();
        assert_eq!(verified.files().len(), 1);
        assert_eq!(
            verified.manifest().app().app_id().as_str(),
            "com.rumahl.examples.notes"
        );
        let _ = fs::remove_dir_all(&downloaded.path);
    }

    #[test]
    fn adds_a_website_source_and_lists_its_apps() {
        let server = serve_index(
            r#"{"formatVersion":1,"name":"Example Store","apps":[{"id":"com.rumahl.notes","title":"Notes","version":"1.2.3","description":"Local notes","publisher":"rumahl","packageUrl":"/packages/notes"}]}"#,
        );
        let state = tempfile::tempdir().unwrap();
        let store = AppStore::open(state.path()).unwrap();
        let source = store.add_source(identity(), "website", &server).unwrap();
        assert_eq!(source.title, "Example Store");
        assert_eq!(source.status, "ready");
        let listing = store.list(identity()).unwrap();
        assert_eq!(listing.sources.len(), 1);
        assert_eq!(listing.apps.len(), 1);
        assert_eq!(listing.apps[0].id, "com.rumahl.notes");
        assert_eq!(listing.apps[0].version, "1.2.3");
        assert_eq!(listing.apps[0].source_id, source.id);
        assert_eq!(listing.apps[0].title, "Notes");
    }

    #[test]
    fn rejects_sources_that_have_no_usable_index() {
        let state = tempfile::tempdir().unwrap();
        let store = AppStore::open(state.path()).unwrap();
        assert_eq!(
            store.add_source(identity(), "website", "file:///etc/passwd"),
            Err(StoreError::InvalidRequest)
        );
        assert_eq!(
            store.add_source(identity(), "website", "http://apps.example.com/store.json"),
            Err(StoreError::InvalidRequest)
        );
        assert!(secure_url(
            &Url::parse("https://apps.example.com/store.json").unwrap()
        ));
        assert!(secure_url(
            &Url::parse("http://127.0.0.1:8787/store.json").unwrap()
        ));
        assert!(!secure_url(
            &Url::parse("http://192.0.2.10/store.json").unwrap()
        ));
        assert_eq!(
            store.add_source(identity(), "github", "owner/repo?bad"),
            Err(StoreError::InvalidRequest)
        );
    }
}
