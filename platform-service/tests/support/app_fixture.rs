//! Integration fixture only: never an application importer or production grant API.
use rumahl_core::*;
use rumahl_persistence_sqlite::{SqliteAccountStateRepository, SqliteSnapshotRepository};
use rumahl_platform_service::apps::{APP_LAUNCH_PERMISSION, app_launch_resource};
use std::path::Path;

pub fn seed(root: &Path, username: &str) -> InstallationId {
    let accounts = SqliteAccountStateRepository::open(root.join("accounts.sqlite"))
        .unwrap()
        .load()
        .unwrap();
    let user = accounts
        .accounts()
        .accounts()
        .iter()
        .find(|user| user.username().as_str() == username)
        .unwrap();
    let repository = SqliteSnapshotRepository::open(root.join("platform.sqlite")).unwrap();
    assert!(
        repository
            .load()
            .unwrap()
            .is_none_or(|s| s.installed_apps().is_empty()),
        "fixture requires an empty platform"
    );
    let mut runtime = RuntimeDescriptor::web();
    runtime
        .add_entrypoint(RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        ))
        .unwrap();
    let manifest = AppManifest::new(
        AppId::parse("com.rumahl.host-test").unwrap(),
        PublisherId::parse("com.rumahl").unwrap(),
        AppVersion::new(1, 0, 0),
        "Isolated test app",
        runtime,
    )
    .unwrap();
    let mut state = PlatformState::new();
    let installed = AppLifecycle::new().install(manifest, &mut state).unwrap();
    let installation = *installed.installation_id();
    let subject = UserIdentity::new(*user.user_id());
    let mut policy = GrantIssuerPolicy::new();
    policy.set_user_role(*user.user_id(), UserRole::Owner);
    let grant = GrantAuthority::new()
        .issue(
            &policy,
            subject.into(),
            subject.into(),
            PermissionId::parse(APP_LAUNCH_PERMISSION).unwrap(),
            PermissionScope::Explicit,
            vec![app_launch_resource(installation)],
        )
        .unwrap();
    let mut grants = InMemoryGrantStore::new();
    grants.insert(grant);
    let assets = root
        .join("app-assets")
        .join(installation.to_string())
        .join("frontend");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("index.html"), r#"<!doctype html><html lang="en"><meta charset="utf-8"><title>Host test</title><body><p id="status">Loading</p><script type="module" src="./main.js"></script></body></html>"#).unwrap();
    std::fs::write(assets.join("main.js"), r#"
let parentBlocked = false, cookieBlocked = false;
try { void parent.document.body; } catch { parentBlocked = true; }
try { void document.cookie; } catch { cookieBlocked = true; }
document.querySelector('#status').textContent = parentBlocked && cookieBlocked ? 'Isolated app ready' : 'Isolation failed';
document.body.dataset.route = location.hash;
"#).unwrap();
    repository
        .store(&PlatformSnapshot::capture(&state, &grants))
        .unwrap();
    installation
}
pub fn revoke(root: &Path) {
    let repository = SqliteSnapshotRepository::open(root.join("platform.sqlite")).unwrap();
    let snapshot = repository.load().unwrap().unwrap();
    assert_eq!(snapshot.installed_apps().len(), 1);
    assert_eq!(
        snapshot.installed_apps()[0].identity().app_id().as_str(),
        "com.rumahl.host-test"
    );
    repository
        .store(&PlatformSnapshot::new(
            snapshot.version(),
            snapshot.installed_apps().to_vec(),
            vec![],
        ))
        .unwrap();
}
