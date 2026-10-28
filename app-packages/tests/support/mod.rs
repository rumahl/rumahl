#![allow(dead_code)]

use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use rumahl_app_packages::{PackageVerificationError, PackageVerifier, TrustStore, VerifiedPackage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

pub fn write_files(root: &Path, files: &[(&str, &[u8])]) {
    for (relative, content) in files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

pub struct Package {
    pub directory: tempfile::TempDir,
    pub signing_key: SigningKey,
    pub key_id: &'static str,
    pub publisher: &'static str,
}

impl Package {
    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn trust_store(&self) -> TrustStore {
        trust_store(&self.signing_key, self.publisher, self.key_id)
    }

    pub fn verify(&self) -> Result<VerifiedPackage, PackageVerificationError> {
        PackageVerifier::new(self.trust_store()).verify(self.root())
    }
}

pub fn trust_store(signing_key: &SigningKey, publisher: &str, key_id: &str) -> TrustStore {
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

pub fn build_package(files: &[(&str, &[u8])]) -> Package {
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
