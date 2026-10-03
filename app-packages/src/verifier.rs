use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Verifier};
use rumahl_core::{AppManifest, PackagePath, PublisherId};

use crate::digest::sha256_file;
use crate::manifest::{
    MANIFEST_FILE, MAX_FILES, MAX_MANIFEST_BYTES, PackageFile, PackageManifest,
    PackageManifestError,
};
use crate::signature::{
    MAX_SIGNATURE_BYTES, SIGNATURE_FILE, SignatureEnvelope, SignatureEnvelopeError, TrustStore,
};

/// Result of verifying one package root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage {
    manifest: PackageManifest,
    signing_key_id: String,
    files: Vec<VerifiedFile>,
}

/// One payload file whose size and digest matched the signed manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFile {
    path: PackagePath,
    size: u64,
    sha256: [u8; 32],
}

#[derive(Debug)]
pub enum PackageVerificationError {
    RootIsNotDirectory,
    DocumentRead {
        file: &'static str,
        source: io::Error,
    },
    DocumentSymlink(&'static str),
    DocumentNotRegular(&'static str),
    DocumentTooLarge(&'static str),
    Manifest(PackageManifestError),
    Signature(SignatureEnvelopeError),
    UnknownSigningKey,
    InvalidSignature,
    Walk {
        path: String,
        source: io::Error,
    },
    NonUtf8Name,
    TooManyFiles,
    SymlinkRejected(String),
    UnsupportedEntry(String),
    UnlistedFile(String),
    MissingFile(String),
    FileRead {
        path: String,
        source: io::Error,
    },
    FileSizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    FileDigestMismatch(String),
}

/// Verifies packages against a fixed trust store.
pub struct PackageVerifier {
    trust_store: TrustStore,
}

struct FoundFile {
    path: String,
    size: u64,
    absolute: PathBuf,
}

impl PackageVerifier {
    pub fn new(trust_store: TrustStore) -> Self {
        Self { trust_store }
    }

    /// Verifies authenticity, completeness and integrity of a package root.
    ///
    /// The manifest signature is checked first, then every payload file is
    /// matched against the signed digest. Unlisted files and symlinks fail
    /// closed. Verification does not publish or mutate anything.
    pub fn verify(&self, root: &Path) -> Result<VerifiedPackage, PackageVerificationError> {
        let metadata = fs::symlink_metadata(root).map_err(|source| {
            PackageVerificationError::DocumentRead {
                file: "package root",
                source,
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(PackageVerificationError::RootIsNotDirectory);
        }

        let manifest_bytes =
            read_document(&root.join(MANIFEST_FILE), MANIFEST_FILE, MAX_MANIFEST_BYTES)?;
        let manifest = PackageManifest::from_bytes(&manifest_bytes)
            .map_err(PackageVerificationError::Manifest)?;

        let signature_bytes = read_document(
            &root.join(SIGNATURE_FILE),
            SIGNATURE_FILE,
            MAX_SIGNATURE_BYTES,
        )?;
        let envelope = SignatureEnvelope::from_bytes(&signature_bytes)
            .map_err(PackageVerificationError::Signature)?;

        let verifying_key = self
            .trust_store
            .verifying_key(manifest.publisher_id(), envelope.key_id())
            .ok_or(PackageVerificationError::UnknownSigningKey)?;
        verifying_key
            .verify(
                &manifest_bytes,
                &Signature::from_bytes(envelope.signature()),
            )
            .map_err(|_| PackageVerificationError::InvalidSignature)?;

        let files = self.verify_payload(root, &manifest)?;

        Ok(VerifiedPackage {
            manifest,
            signing_key_id: envelope.key_id().to_owned(),
            files,
        })
    }

    fn verify_payload(
        &self,
        root: &Path,
        manifest: &PackageManifest,
    ) -> Result<Vec<VerifiedFile>, PackageVerificationError> {
        let mut found = Vec::new();
        collect_payload_files(root, "", &mut found)?;

        let mut listed: HashMap<&str, &PackageFile> = manifest
            .files()
            .iter()
            .map(|file| (file.path().as_str(), file))
            .collect();

        let mut verified = Vec::with_capacity(found.len());
        for file in &found {
            let Some(expected) = listed.remove(file.path.as_str()) else {
                return Err(PackageVerificationError::UnlistedFile(file.path.clone()));
            };

            if file.size != expected.size() {
                return Err(PackageVerificationError::FileSizeMismatch {
                    path: file.path.clone(),
                    expected: expected.size(),
                    actual: file.size,
                });
            }

            let digest = sha256_file(&file.absolute).map_err(|source| {
                PackageVerificationError::FileRead {
                    path: file.path.clone(),
                    source,
                }
            })?;
            if &digest != expected.sha256() {
                return Err(PackageVerificationError::FileDigestMismatch(
                    file.path.clone(),
                ));
            }

            verified.push(VerifiedFile {
                path: expected.path().clone(),
                size: expected.size(),
                sha256: digest,
            });
        }

        if let Some(path) = listed.keys().min() {
            return Err(PackageVerificationError::MissingFile((*path).to_owned()));
        }

        Ok(verified)
    }
}

impl VerifiedPackage {
    pub fn manifest(&self) -> &PackageManifest {
        &self.manifest
    }

    pub fn publisher_id(&self) -> &PublisherId {
        self.manifest.publisher_id()
    }

    pub fn signing_key_id(&self) -> &str {
        &self.signing_key_id
    }

    pub fn files(&self) -> &[VerifiedFile] {
        &self.files
    }

    pub fn to_app_manifest(&self) -> Result<AppManifest, PackageManifestError> {
        self.manifest.to_app_manifest()
    }
}

impl VerifiedFile {
    pub fn path(&self) -> &PackagePath {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

fn read_document(
    path: &Path,
    file: &'static str,
    max_bytes: usize,
) -> Result<Vec<u8>, PackageVerificationError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| PackageVerificationError::DocumentRead { file, source })?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(PackageVerificationError::DocumentSymlink(file));
    }
    if !file_type.is_file() {
        return Err(PackageVerificationError::DocumentNotRegular(file));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(PackageVerificationError::DocumentTooLarge(file));
    }

    fs::read(path).map_err(|source| PackageVerificationError::DocumentRead { file, source })
}

fn collect_payload_files(
    directory: &Path,
    prefix: &str,
    found: &mut Vec<FoundFile>,
) -> Result<(), PackageVerificationError> {
    if found.len() > MAX_FILES {
        return Err(PackageVerificationError::TooManyFiles);
    }

    let entries = fs::read_dir(directory).map_err(|source| PackageVerificationError::Walk {
        path: display_path(directory),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| PackageVerificationError::Walk {
            path: display_path(directory),
            source,
        })?;

        let file_name = entry.file_name();
        let name = file_name
            .to_str()
            .ok_or(PackageVerificationError::NonUtf8Name)?;
        let relative = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };

        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            PackageVerificationError::Walk {
                path: relative.clone(),
                source,
            }
        })?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Err(PackageVerificationError::SymlinkRejected(relative));
        }

        if file_type.is_dir() {
            collect_payload_files(&entry.path(), &relative, found)?;
        } else if file_type.is_file() {
            if prefix.is_empty() && (name == MANIFEST_FILE || name == SIGNATURE_FILE) {
                continue;
            }

            if found.len() >= MAX_FILES {
                return Err(PackageVerificationError::TooManyFiles);
            }

            found.push(FoundFile {
                path: relative,
                size: metadata.len(),
                absolute: entry.path(),
            });
        } else {
            return Err(PackageVerificationError::UnsupportedEntry(relative));
        }
    }

    Ok(())
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl fmt::Display for PackageVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootIsNotDirectory => write!(f, "package root is not a directory"),
            Self::DocumentRead { file, source } => {
                write!(f, "package {file} could not be read: {source}")
            }
            Self::DocumentSymlink(file) => write!(f, "package {file} must not be a symlink"),
            Self::DocumentNotRegular(file) => write!(f, "package {file} must be a regular file"),
            Self::DocumentTooLarge(file) => write!(f, "package {file} exceeds the size limit"),
            Self::Manifest(error) => write!(f, "package manifest is invalid: {error}"),
            Self::Signature(error) => write!(f, "package signature is invalid: {error}"),
            Self::UnknownSigningKey => {
                write!(f, "package is signed by an unknown or untrusted key")
            }
            Self::InvalidSignature => write!(f, "package signature does not verify"),
            Self::Walk { path, source } => {
                write!(f, "package path '{path}' could not be read: {source}")
            }
            Self::NonUtf8Name => write!(f, "package contains a non-UTF-8 file name"),
            Self::TooManyFiles => write!(f, "package contains more files than the limit"),
            Self::SymlinkRejected(path) => {
                write!(f, "package path '{path}' must not be a symlink")
            }
            Self::UnsupportedEntry(path) => {
                write!(
                    f,
                    "package path '{path}' is not a regular file or directory"
                )
            }
            Self::UnlistedFile(path) => {
                write!(f, "package file '{path}' is not listed in the manifest")
            }
            Self::MissingFile(path) => {
                write!(f, "package file '{path}' is listed but missing")
            }
            Self::FileRead { path, source } => {
                write!(f, "package file '{path}' could not be read: {source}")
            }
            Self::FileSizeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "package file '{path}' has {actual} bytes but the manifest declares {expected}"
            ),
            Self::FileDigestMismatch(path) => {
                write!(
                    f,
                    "package file '{path}' does not match its manifest digest"
                )
            }
        }
    }
}

impl Error for PackageVerificationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DocumentRead { source, .. } => Some(source),
            Self::Manifest(error) => Some(error),
            Self::Signature(error) => Some(error),
            Self::Walk { source, .. } => Some(source),
            Self::FileRead { source, .. } => Some(source),
            Self::RootIsNotDirectory
            | Self::DocumentSymlink(_)
            | Self::DocumentNotRegular(_)
            | Self::DocumentTooLarge(_)
            | Self::UnknownSigningKey
            | Self::InvalidSignature
            | Self::NonUtf8Name
            | Self::TooManyFiles
            | Self::SymlinkRejected(_)
            | Self::UnsupportedEntry(_)
            | Self::UnlistedFile(_)
            | Self::MissingFile(_)
            | Self::FileSizeMismatch { .. }
            | Self::FileDigestMismatch(_) => None,
        }
    }
}
