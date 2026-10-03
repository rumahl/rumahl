use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use rumahl_app_packages::{
    PackageManifest, PackageVerificationError, PackageVerifier, TrustStore, VerifiedPackage,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn write_files(root: &Path, files: &[(&str, &[u8])]) {
    for (relative, content) in files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

struct Package {
    directory: tempfile::TempDir,
    signing_key: SigningKey,
    key_id: &'static str,
    publisher: &'static str,
}

impl Package {
    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn trust_store(&self) -> TrustStore {
        trust_store(&self.signing_key, self.publisher, self.key_id)
    }

    fn verify(&self) -> Result<VerifiedPackage, PackageVerificationError> {
        PackageVerifier::new(self.trust_store()).verify(self.root())
    }
}

fn trust_store(signing_key: &SigningKey, publisher: &str, key_id: &str) -> TrustStore {
    let value = json!({
        "formatVersion": 1,
        "keys": [{
            "keyId": key_id,
            "publisherId": publisher,
            "publicKey": URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
        }],
    });
    TrustStore::from_bytes(&serde_json::to_vec(&value).unwrap()).unwrap()
}

fn build_package(files: &[(&str, &[u8])]) -> Package {
    let directory = tempfile::tempdir().unwrap();
    write_files(directory.path(), files);

    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let key_id = "publisher-key-1";
    let publisher = "com.rumahl";

    let entries = files
        .iter()
        .map(|(relative, content)| {
            json!({
                "path": relative,
                "size": content.len(),
                "sha256": hex(&Sha256::digest(content)),
            })
        })
        .collect::<Vec<Value>>();

    let manifest = json!({
        "formatVersion": 1,
        "publisherId": publisher,
        "app": {
            "appId": "com.rumahl.notes",
            "version": "1.0.0",
            "displayName": "Notes",
            "runtime": {
                "kind": "web",
                "entrypoints": [
                    { "id": "main", "kind": "web-asset", "path": "frontend/index.html" },
                ],
            },
        },
        "files": entries,
    });
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(directory.path().join("package.json"), &manifest_bytes).unwrap();

    let signature = signing_key.sign(&manifest_bytes);
    let envelope = json!({
        "formatVersion": 1,
        "algorithm": "ed25519",
        "keyId": key_id,
        "signature": URL_SAFE_NO_PAD.encode(signature.to_bytes()),
    });
    fs::write(
        directory.path().join("package.sig.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .unwrap();

    Package {
        directory,
        signing_key,
        key_id,
        publisher,
    }
}

#[test]
fn verifies_a_signed_package() {
    let package = build_package(&[
        ("frontend/index.html", b"<html>notes</html>"),
        ("frontend/app.js", b"console.log('notes')"),
    ]);

    let verified = package.verify().unwrap();

    assert_eq!(verified.publisher_id().as_str(), "com.rumahl");
    assert_eq!(verified.signing_key_id(), "publisher-key-1");
    assert_eq!(verified.files().len(), 2);
    assert_eq!(verified.manifest().app().display_name(), "Notes");
    assert_eq!(
        verified.manifest().app().runtime().kind(),
        rumahl_core::RuntimeKind::Web
    );
    assert_eq!(
        verified
            .manifest()
            .to_app_manifest()
            .unwrap()
            .app_id()
            .as_str(),
        "com.rumahl.notes"
    );
}

#[test]
fn rejects_tampered_payload() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);

    fs::write(
        package.root().join("frontend/index.html"),
        b"<html>evil</html>",
    )
    .unwrap();

    assert!(matches!(
        package.verify().unwrap_err(),
        PackageVerificationError::FileSizeMismatch { .. }
            | PackageVerificationError::FileDigestMismatch(_)
    ));
}

#[test]
fn rejects_tampered_manifest() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);

    let mut manifest = fs::read(package.root().join("package.json")).unwrap();
    manifest.extend_from_slice(b" ");
    fs::write(package.root().join("package.json"), manifest).unwrap();

    assert!(matches!(
        package.verify().unwrap_err(),
        PackageVerificationError::InvalidSignature
    ));
}

#[test]
fn rejects_unknown_signing_key() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    let other = SigningKey::from_bytes(&[9_u8; 32]);

    let store = trust_store(&other, package.publisher, package.key_id);
    let error = PackageVerifier::new(store)
        .verify(package.root())
        .unwrap_err();

    assert!(matches!(error, PackageVerificationError::InvalidSignature));
}

#[test]
fn rejects_key_for_another_publisher() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);

    let store = trust_store(&package.signing_key, "com.example", package.key_id);
    let error = PackageVerifier::new(store)
        .verify(package.root())
        .unwrap_err();

    assert!(matches!(error, PackageVerificationError::UnknownSigningKey));
}

#[test]
fn rejects_unlisted_file() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    fs::write(package.root().join("frontend/extra.js"), b"extra").unwrap();

    assert!(matches!(
        package.verify().unwrap_err(),
        PackageVerificationError::UnlistedFile(path) if path == "frontend/extra.js"
    ));
}

#[test]
fn rejects_missing_file() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    fs::remove_file(package.root().join("frontend/index.html")).unwrap();

    assert!(matches!(
        package.verify().unwrap_err(),
        PackageVerificationError::MissingFile(path) if path == "frontend/index.html"
    ));
}

#[test]
fn rejects_symlinked_payload() {
    let package = build_package(&[("frontend/index.html", b"<html>notes</html>")]);
    symlink(
        package.root().join("frontend/index.html"),
        package.root().join("frontend/link.html"),
    )
    .unwrap();

    assert!(matches!(
        package.verify().unwrap_err(),
        PackageVerificationError::SymlinkRejected(path) if path == "frontend/link.html"
    ));
}

#[test]
fn rejects_path_traversal_in_manifest() {
    let manifest = json!({
        "formatVersion": 1,
        "publisherId": "com.rumahl",
        "app": {
            "appId": "com.rumahl.notes",
            "version": "1.0.0",
            "displayName": "Notes",
            "runtime": { "kind": "web", "entrypoints": [] },
        },
        "files": [{ "path": "../secret", "size": 0, "sha256": "0".repeat(64) }],
    });

    let error = PackageManifest::from_bytes(&serde_json::to_vec(&manifest).unwrap()).unwrap_err();
    assert!(matches!(
        error,
        rumahl_app_packages::PackageManifestError::InvalidFilePath
    ));
}

#[test]
fn rejects_unknown_manifest_fields() {
    let manifest = json!({
        "formatVersion": 1,
        "publisherId": "com.rumahl",
        "app": {
            "appId": "com.rumahl.notes",
            "version": "1.0.0",
            "displayName": "Notes",
            "runtime": { "kind": "web", "entrypoints": [] },
        },
        "files": [],
        "unexpected": true,
    });

    assert!(matches!(
        PackageManifest::from_bytes(&serde_json::to_vec(&manifest).unwrap()).unwrap_err(),
        rumahl_app_packages::PackageManifestError::Malformed(_)
    ));
}

#[test]
fn rejects_unsupported_format_version() {
    let manifest = json!({
        "formatVersion": 2,
        "publisherId": "com.rumahl",
        "app": {
            "appId": "com.rumahl.notes",
            "version": "1.0.0",
            "displayName": "Notes",
            "runtime": { "kind": "web", "entrypoints": [] },
        },
        "files": [],
    });

    assert!(matches!(
        PackageManifest::from_bytes(&serde_json::to_vec(&manifest).unwrap()).unwrap_err(),
        rumahl_app_packages::PackageManifestError::UnsupportedFormatVersion(2)
    ));
}
