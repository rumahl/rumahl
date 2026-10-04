use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use rumahl_core::{InstallationId, RuntimeEntrypointTarget, RuntimeKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::digest::{sha256_file, to_hex};
use crate::manifest::{
    MANIFEST_FILE, MAX_FILE_BYTES, MAX_PACKAGE_BYTES, PackageManifest, PackageManifestError,
};
use crate::signature::{SIGNATURE_FILE, valid_key_id};
use crate::verifier::{PackageVerificationError, collect_payload_files};

const KEYSTORE_TEMP_MODE: u32 = 0o600;
const KEYSTORE_FORMAT_VERSION: u32 = 1;
const ED25519_SEED_LENGTH: usize = 32;

/// Builds and signs a package manifest over the payload files under a root.
pub struct PackageSigner {
    key_id: String,
    signing_key: SigningKey,
}

/// A freshly generated Ed25519 key pair for package signing.
pub struct GeneratedKey {
    key_id: String,
    seed: [u8; ED25519_SEED_LENGTH],
}

/// A persistable signing key: key id plus the 32-byte Ed25519 seed.
pub struct KeyStore {
    key_id: String,
    seed: [u8; ED25519_SEED_LENGTH],
}

#[derive(Debug)]
pub enum PackageSignError {
    RootIsNotDirectory,
    Collection(PackageVerificationError),
    FileRead { path: String, source: io::Error },
    FileTooLarge(String),
    PackageTooLarge,
    Serialize(serde_json::Error),
    InvalidManifest(PackageManifestError),
    Write(io::Error),
    Random(getrandom::Error),
    InvalidKeyId,
    MalformedKeyStore(serde_json::Error),
    UnsupportedKeyStoreVersion,
    InvalidSeedEncoding(base64::DecodeError),
    InvalidSeedLength,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawKeyStore {
    format_version: u32,
    key_id: String,
    seed: String,
}

impl PackageSigner {
    pub fn from_seed(
        key_id: impl Into<String>,
        seed: &[u8; ED25519_SEED_LENGTH],
    ) -> Result<Self, PackageSignError> {
        let key_id = key_id.into();
        if !valid_key_id(&key_id) {
            return Err(PackageSignError::InvalidKeyId);
        }
        Ok(Self {
            key_id,
            signing_key: SigningKey::from_bytes(seed),
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_key(&self) -> [u8; ED25519_SEED_LENGTH] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Writes `package.json` and `package.sig.json` into `root`.
    ///
    /// File digests are recomputed from the current payload, so the caller's
    /// template only supplies identity and runtime metadata. The generated
    /// manifest is re-parsed before it is written, and both documents are
    /// published atomically.
    pub fn sign(&self, root: &Path, template: &PackageManifest) -> Result<(), PackageSignError> {
        let metadata = fs::symlink_metadata(root)
            .map_err(|source| PackageSignError::Write(io::Error::new(source.kind(), source)))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(PackageSignError::RootIsNotDirectory);
        }

        let mut found = Vec::new();
        collect_payload_files(root, "", &mut found).map_err(PackageSignError::Collection)?;
        found.sort_by(|left, right| left.path.cmp(&right.path));

        let mut files = Vec::with_capacity(found.len());
        let mut total = 0_u64;
        for file in &found {
            if file.size > MAX_FILE_BYTES {
                return Err(PackageSignError::FileTooLarge(file.path.clone()));
            }
            total = total.saturating_add(file.size);
            if total > MAX_PACKAGE_BYTES {
                return Err(PackageSignError::PackageTooLarge);
            }
            let digest =
                sha256_file(&file.absolute).map_err(|source| PackageSignError::FileRead {
                    path: file.path.clone(),
                    source,
                })?;
            files.push(json!({
                "path": file.path,
                "size": file.size,
                "sha256": to_hex(&digest),
            }));
        }

        let manifest_bytes = serde_json::to_vec(&manifest_value(template, &files))
            .map_err(PackageSignError::Serialize)?;
        PackageManifest::from_bytes(&manifest_bytes).map_err(PackageSignError::InvalidManifest)?;
        write_atomic(&root.join(MANIFEST_FILE), &manifest_bytes)?;

        let signature = self.signing_key.sign(&manifest_bytes);
        let envelope = json!({
            "formatVersion": 1,
            "algorithm": "ed25519",
            "keyId": self.key_id,
            "signature": URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        });
        let envelope_bytes = serde_json::to_vec(&envelope).map_err(PackageSignError::Serialize)?;
        write_atomic(&root.join(SIGNATURE_FILE), &envelope_bytes)
    }
}

impl GeneratedKey {
    pub fn generate(key_id: impl Into<String>) -> Result<Self, PackageSignError> {
        let key_id = key_id.into();
        if !valid_key_id(&key_id) {
            return Err(PackageSignError::InvalidKeyId);
        }
        let mut seed = [0_u8; ED25519_SEED_LENGTH];
        getrandom::fill(&mut seed).map_err(PackageSignError::Random)?;
        Ok(Self { key_id, seed })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn seed(&self) -> &[u8; ED25519_SEED_LENGTH] {
        &self.seed
    }

    pub fn public_key(&self) -> [u8; ED25519_SEED_LENGTH] {
        SigningKey::from_bytes(&self.seed)
            .verifying_key()
            .to_bytes()
    }

    pub fn key_store(&self) -> KeyStore {
        KeyStore {
            key_id: self.key_id.clone(),
            seed: self.seed,
        }
    }
}

impl KeyStore {
    pub fn new(
        key_id: impl Into<String>,
        seed: [u8; ED25519_SEED_LENGTH],
    ) -> Result<Self, PackageSignError> {
        let key_id = key_id.into();
        if !valid_key_id(&key_id) {
            return Err(PackageSignError::InvalidKeyId);
        }
        Ok(Self { key_id, seed })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, PackageSignError> {
        let raw: RawKeyStore =
            serde_json::from_slice(bytes).map_err(PackageSignError::MalformedKeyStore)?;
        if raw.format_version != KEYSTORE_FORMAT_VERSION {
            return Err(PackageSignError::UnsupportedKeyStoreVersion);
        }
        if !valid_key_id(&raw.key_id) {
            return Err(PackageSignError::InvalidKeyId);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(&raw.seed)
            .map_err(PackageSignError::InvalidSeedEncoding)?;
        let seed: [u8; ED25519_SEED_LENGTH] = decoded
            .try_into()
            .map_err(|_| PackageSignError::InvalidSeedLength)?;
        Ok(Self {
            key_id: raw.key_id,
            seed,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn seed(&self) -> &[u8; ED25519_SEED_LENGTH] {
        &self.seed
    }

    pub fn encode(&self) -> Result<Vec<u8>, PackageSignError> {
        serde_json::to_vec(&RawKeyStore {
            format_version: KEYSTORE_FORMAT_VERSION,
            key_id: self.key_id.clone(),
            seed: URL_SAFE_NO_PAD.encode(self.seed),
        })
        .map_err(PackageSignError::Serialize)
    }

    pub fn signer(&self) -> Result<PackageSigner, PackageSignError> {
        PackageSigner::from_seed(self.key_id.clone(), &self.seed)
    }
}

/// Writes a key store to `path` with owner-only permissions.
pub fn write_key_store(path: &Path, store: &KeyStore) -> Result<(), PackageSignError> {
    let bytes = store.encode()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(KEYSTORE_TEMP_MODE)
        .open(path)
        .map_err(PackageSignError::Write)?;
    file.write_all(&bytes).map_err(PackageSignError::Write)?;
    file.sync_all().map_err(PackageSignError::Write)
}

/// Builds a trust store document for one publisher key.
pub fn trust_store_json(
    key_id: &str,
    publisher_id: &str,
    public_key: &[u8; ED25519_SEED_LENGTH],
) -> Value {
    json!({
        "formatVersion": 1,
        "keys": [{
            "keyId": key_id,
            "publisherId": publisher_id,
            "publicKey": URL_SAFE_NO_PAD.encode(public_key),
        }],
    })
}

fn manifest_value(manifest: &PackageManifest, files: &[Value]) -> Value {
    let runtime = manifest.app().runtime();
    let entrypoints = runtime
        .entrypoints()
        .iter()
        .map(|entrypoint| {
            let id = entrypoint.id().as_str();
            match entrypoint.target() {
                RuntimeEntrypointTarget::WebAsset(path) => {
                    json!({ "id": id, "kind": "web-asset", "path": path.as_str() })
                }
                RuntimeEntrypointTarget::ContainerArtifact(path) => {
                    json!({ "id": id, "kind": "container-artifact", "path": path.as_str() })
                }
                RuntimeEntrypointTarget::Endpoint(endpoint) => {
                    json!({ "id": id, "kind": "endpoint", "endpoint": endpoint.as_str() })
                }
            }
        })
        .collect::<Vec<_>>();

    json!({
        "formatVersion": manifest.format_version(),
        "publisherId": manifest.publisher_id().as_str(),
        "app": {
            "appId": manifest.app().app_id().as_str(),
            "version": manifest.app().version().to_string(),
            "displayName": manifest.app().display_name(),
            "runtime": {
                "kind": runtime_kind_str(runtime.kind()),
                "entrypoints": entrypoints,
            },
        },
        "files": files,
    })
}

fn runtime_kind_str(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Web => "web",
        RuntimeKind::Container => "container",
        RuntimeKind::Native => "native",
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PackageSignError> {
    let parent: PathBuf = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package");
    let temporary = parent.join(format!(".{file_name}.tmp-{}", InstallationId::new()));

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(PackageSignError::Write)?;
        file.write_all(bytes).map_err(PackageSignError::Write)?;
        file.sync_all().map_err(PackageSignError::Write)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(PackageSignError::Write(error));
    }
    Ok(())
}

impl fmt::Display for PackageSignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootIsNotDirectory => write!(f, "package root is not a directory"),
            Self::Collection(error) => {
                write!(f, "package payload could not be enumerated: {error}")
            }
            Self::FileRead { path, source } => {
                write!(f, "package file '{path}' could not be read: {source}")
            }
            Self::FileTooLarge(path) => write!(f, "package file '{path}' exceeds the size limit"),
            Self::PackageTooLarge => write!(f, "package payload exceeds the size limit"),
            Self::Serialize(error) => write!(f, "package manifest could not be encoded: {error}"),
            Self::InvalidManifest(error) => {
                write!(f, "generated package manifest is invalid: {error}")
            }
            Self::Write(error) => write!(f, "package document could not be written: {error}"),
            Self::Random(error) => write!(f, "signing key could not be generated: {error}"),
            Self::InvalidKeyId => write!(f, "signing key id is invalid"),
            Self::MalformedKeyStore(error) => write!(f, "key store is malformed: {error}"),
            Self::UnsupportedKeyStoreVersion => {
                write!(f, "key store format version is not supported")
            }
            Self::InvalidSeedEncoding(error) => {
                write!(f, "key store seed is not valid base64: {error}")
            }
            Self::InvalidSeedLength => write!(f, "key store seed must decode to 32 bytes"),
        }
    }
}

impl Error for PackageSignError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Collection(error) => Some(error),
            Self::FileRead { source, .. } => Some(source),
            Self::Serialize(error) => Some(error),
            Self::InvalidManifest(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::Random(error) => Some(error),
            Self::MalformedKeyStore(error) => Some(error),
            Self::InvalidSeedEncoding(error) => Some(error),
            Self::RootIsNotDirectory
            | Self::FileTooLarge(_)
            | Self::PackageTooLarge
            | Self::InvalidKeyId
            | Self::UnsupportedKeyStoreVersion
            | Self::InvalidSeedLength => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::signature::TrustStore;
    use crate::verifier::PackageVerifier;

    use super::*;

    fn template(runtime_kind: &str) -> PackageManifest {
        let runtime = match runtime_kind {
            "web" => {
                r#"{ "kind": "web", "entrypoints": [ { "id": "main", "kind": "web-asset", "path": "frontend/index.html" } ] }"#
            }
            _ => {
                r#"{ "kind": "container", "entrypoints": [ { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" }, { "id": "main", "kind": "endpoint", "endpoint": "web" } ] }"#
            }
        };
        let value = json!({
            "formatVersion": 1,
            "publisherId": "com.rumahl",
            "app": {
                "appId": "com.rumahl.notes",
                "version": "1.0.0",
                "displayName": "Notes",
                "runtime": serde_json::from_str::<Value>(runtime).unwrap(),
            },
        });
        PackageManifest::from_bytes(&serde_json::to_vec(&value).unwrap()).unwrap()
    }

    fn key() -> PackageSigner {
        PackageSigner::from_seed("key-1", &[7_u8; 32]).unwrap()
    }

    #[test]
    fn signs_and_verifies_a_web_package() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("frontend")).unwrap();
        fs::write(
            directory.path().join("frontend/index.html"),
            b"<html>notes</html>",
        )
        .unwrap();

        key().sign(directory.path(), &template("web")).unwrap();

        let trust = trust_store_json("key-1", "com.rumahl", &key().public_key()).to_string();
        let trust = TrustStore::from_bytes(trust.as_bytes()).unwrap();
        let verified = PackageVerifier::new(trust)
            .verify(directory.path())
            .unwrap();

        assert_eq!(verified.files().len(), 1);
        assert_eq!(verified.signing_key_id(), "key-1");
    }

    #[test]
    fn recomputes_digests_after_content_change() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("runtime")).unwrap();
        fs::write(directory.path().join("runtime/server.oci"), b"first").unwrap();

        let signer = key();
        signer
            .sign(directory.path(), &template("container"))
            .unwrap();

        fs::write(directory.path().join("runtime/server.oci"), b"second").unwrap();
        signer
            .sign(directory.path(), &template("container"))
            .unwrap();

        let trust = trust_store_json("key-1", "com.rumahl", &signer.public_key()).to_string();
        let trust = TrustStore::from_bytes(trust.as_bytes()).unwrap();
        PackageVerifier::new(trust)
            .verify(directory.path())
            .unwrap();
    }

    #[test]
    fn rejects_symlinked_payload_when_signing() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("frontend")).unwrap();
        let outside = directory.path().join("outside.html");
        fs::write(&outside, b"<html>outside</html>").unwrap();
        std::os::unix::fs::symlink(&outside, directory.path().join("frontend/index.html")).unwrap();

        let error = key().sign(directory.path(), &template("web")).unwrap_err();
        assert!(matches!(error, PackageSignError::Collection(_)));
    }

    #[test]
    fn key_store_round_trips() {
        let generated = GeneratedKey::generate("key-1").unwrap();
        let encoded = generated.key_store().encode().unwrap();
        let parsed = KeyStore::parse(&encoded).unwrap();

        assert_eq!(parsed.key_id(), "key-1");
        assert_eq!(parsed.seed(), generated.seed());
    }
}
