//! Signed application package format and offline verification.
//!
//! A package is a directory containing a signed [`PackageManifest`]
//! (`package.json`), a detached Ed25519 signature (`package.sig.json`) and the
//! immutable payload files the manifest enumerates and hashes.
//!
//! Verification is deliberately split from publication. This crate answers only
//! whether a package is authentic, complete and internally consistent. Importing
//! or publishing the verified payload is a separate, later participant.
//!
//! ```text
//! <package-root>/
//!   package.json       signed manifest (app identity, entrypoints, file hashes)
//!   package.sig.json   detached Ed25519 signature over package.json
//!   frontend/index.html
//!   runtime/server.oci
//! ```

mod digest;
mod manifest;
mod signature;
mod verifier;

pub use manifest::{
    MANIFEST_FILE, MAX_FILE_BYTES, MAX_FILES, MAX_MANIFEST_BYTES, MAX_PACKAGE_BYTES,
    PackageAppManifest, PackageFile, PackageManifest, PackageManifestError,
};
pub use signature::{
    MAX_SIGNATURE_BYTES, MAX_TRUST_STORE_BYTES, SIGNATURE_FILE, SignatureEnvelope,
    SignatureEnvelopeError, TrustStore, TrustStoreError, TrustedKey,
};
pub use verifier::{PackageVerificationError, PackageVerifier, VerifiedFile, VerifiedPackage};
