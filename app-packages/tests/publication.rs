mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use rumahl_app_packages::{WebAssetPublishError, WebAssetPublisher, WebAssetPublisherConfig};
use rumahl_core::InstallationId;

use support::build_package;

fn publisher(root: &Path) -> WebAssetPublisher {
    WebAssetPublisher::new(WebAssetPublisherConfig::new(root).unwrap())
}

#[test]
fn publishes_verified_assets_under_installation_root() {
    let package = build_package(&[
        ("frontend/index.html", b"<html>notes</html>"),
        ("frontend/app.js", b"console.log('notes')"),
    ]);
    let verified = package.verify().unwrap();

    let assets = tempfile::tempdir().unwrap();
    let installation_id = InstallationId::new();
    let published = publisher(assets.path())
        .publish(&installation_id, package.root(), &verified)
        .unwrap();

    assert_eq!(published.installation_id(), &installation_id);
    assert_eq!(published.file_count(), 2);
    assert_eq!(
        published.total_bytes(),
        "<html>notes</html>".len() as u64 + "console.log('notes')".len() as u64
    );
    assert_eq!(
        fs::read(published.path().join("frontend/index.html")).unwrap(),
        b"<html>notes</html>"
    );
    assert_eq!(
        fs::read(published.path().join("frontend/app.js")).unwrap(),
        b"console.log('notes')"
    );
}

#[test]
fn refuses_to_overwrite_a_published_installation() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    let verified = package.verify().unwrap();

    let assets = tempfile::tempdir().unwrap();
    let installation_id = InstallationId::new();
    let publisher = publisher(assets.path());
    publisher
        .publish(&installation_id, package.root(), &verified)
        .unwrap();

    let error = publisher
        .publish(&installation_id, package.root(), &verified)
        .unwrap_err();

    assert!(matches!(error, WebAssetPublishError::AlreadyPublished(_)));
}

#[test]
fn rejects_payload_swapped_after_verification() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    let verified = package.verify().unwrap();

    // Same length, different content: a pure digest mismatch.
    fs::write(
        package.root().join("frontend/index.html"),
        b"<html>evil!</html>",
    )
    .unwrap();

    let assets = tempfile::tempdir().unwrap();
    let installation_id = InstallationId::new();
    let error = publisher(assets.path())
        .publish(&installation_id, package.root(), &verified)
        .unwrap_err();

    assert!(matches!(error, WebAssetPublishError::DigestMismatch(_)));
    assert!(!assets.path().join(installation_id.to_string()).exists());
}

#[test]
fn rejects_symlinked_payload_at_publication() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    let verified = package.verify().unwrap();

    let outside = package.root().join("outside.html");
    fs::write(&outside, b"<html>outside</html>").unwrap();
    fs::remove_file(package.root().join("frontend/index.html")).unwrap();
    symlink(&outside, package.root().join("frontend/index.html")).unwrap();

    let assets = tempfile::tempdir().unwrap();
    let installation_id = InstallationId::new();
    let error = publisher(assets.path())
        .publish(&installation_id, package.root(), &verified)
        .unwrap_err();

    assert!(matches!(error, WebAssetPublishError::SourceSymlink(_)));
    assert!(!assets.path().join(installation_id.to_string()).exists());
}
