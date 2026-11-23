//! End-to-end: resolve a website source, download a signed package and install it.
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use rumahl_app_packages::{
    GeneratedKey, PackageManifest, PackageVerifier, TrustStore, trust_store_json,
};
use rumahl_core::{PlatformSnapshotRepository, SessionId, UserId};
use rumahl_persistence_sqlite::SqliteSnapshotRepository;
use rumahl_platform_service::store::AppStore;
use rumahl_platform_web::{ShellIdentity, StoreProvider};
use serde_json::json;

fn serve_directory(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(32) {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            if reader.read_line(&mut request).unwrap_or(0) == 0 {
                continue;
            }
            let path = request
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
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
fn installs_a_signed_app_from_a_website_source() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    fs::create_dir(&state).unwrap();
    let package = root.path().join("site/packages/notes");
    fs::create_dir_all(package.join("frontend")).unwrap();
    fs::write(package.join("frontend/index.html"), b"<h1>Notes</h1>").unwrap();
    let generated = GeneratedKey::generate("test-key").unwrap();
    let trust_store = serde_json::to_vec(&trust_store_json(
        "test-key",
        "com.rumahl.examples",
        &generated.public_key(),
    ))
    .unwrap();
    fs::write(root.path().join("trust.json"), &trust_store).unwrap();
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
    let manifest = PackageManifest::from_bytes(&serde_json::to_vec(&template).unwrap()).unwrap();
    generated
        .key_store()
        .signer()
        .unwrap()
        .sign(&package, &manifest)
        .unwrap();
    let verifier = PackageVerifier::new(TrustStore::from_bytes(&trust_store).unwrap());
    verifier.verify(&package).unwrap();
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
    let socket = root.path().join("runtime.sock");
    let variables: [(&str, String); 12] = [
        ("RUMAHL_RUNTIME_UID", "0".to_owned()),
        (
            "RUMAHL_RUNTIME_CONTROL_SOCKET",
            socket.to_string_lossy().into_owned(),
        ),
        (
            "RUMAHL_RUNTIME_SECRET_SOCKET",
            socket.to_string_lossy().into_owned(),
        ),
        (
            "RUMAHL_RUNTIME_IMAGE_SOCKET",
            socket.to_string_lossy().into_owned(),
        ),
        (
            "RUMAHL_PACKAGE_TRUST_STORE",
            root.path()
                .join("trust.json")
                .to_string_lossy()
                .into_owned(),
        ),
        ("RUMAHL_TPM2_EXECUTABLE", "/bin/true".to_owned()),
        ("RUMAHL_SECRET_KEY_ID", "test-root".to_owned()),
        (
            "RUMAHL_SECRET_KEY_OBJECT",
            root.path().join("root.ctx").to_string_lossy().into_owned(),
        ),
        ("RUMAHL_APP_HOST_SUFFIX", "apps.localhost".to_owned()),
        (
            "RUMAHL_APPS_ROOT",
            root.path().join("apps").to_string_lossy().into_owned(),
        ),
        ("RUMAHL_PUBLIC_ORIGIN", "https://localhost:8443".to_owned()),
        (
            "RUMAHL_GITHUB_RAW_BASE",
            "https://raw.githubusercontent.com".to_owned(),
        ),
    ];
    for (name, value) in &variables {
        // Safety: this integration-test binary has exactly one test and no
        // other thread reads these process variables concurrently.
        unsafe { std::env::set_var(name, value) };
    }

    let store = AppStore::open(&state).unwrap();
    let identity = ShellIdentity {
        user_id: UserId::new(),
        session_id: SessionId::new(),
    };
    let source = store
        .add_source(identity, "website", &format!("{server}/site/store.json"))
        .unwrap();
    let listing = store.list(identity).unwrap();
    assert_eq!(listing.apps.len(), 1);
    assert_eq!(listing.apps[0].title, "Notes");
    let installed = store
        .install(identity, &source.id, "com.rumahl.examples.notes")
        .unwrap();
    assert_eq!(installed.id, "com.rumahl.examples.notes");
    assert_eq!(installed.title, "Notes");

    let snapshot = SqliteSnapshotRepository::open(state.join("platform.sqlite"))
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.installed_apps().len(), 1);
    let assets = state
        .join("app-assets")
        .join(installed.installation_id.to_string())
        .join("frontend/index.html");
    assert_eq!(fs::read(assets).unwrap(), b"<h1>Notes</h1>");

    for (name, _) in &variables {
        unsafe { std::env::remove_var(name) };
    }
}
