use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::VerifyingKey;
use rumahl_core::PublisherId;
use serde::Deserialize;

/// File name of the detached signature envelope inside a package root.
pub const SIGNATURE_FILE: &str = "package.sig.json";

/// Upper bound for the signature envelope document.
pub const MAX_SIGNATURE_BYTES: usize = 4 * 1024;

/// Upper bound for a trust store document.
pub const MAX_TRUST_STORE_BYTES: usize = 1024 * 1024;

const FORMAT_VERSION: u32 = 1;
const SIGNATURE_ALGORITHM: &str = "ed25519";
const MAX_KEY_ID_LENGTH: usize = 64;
const ED25519_PUBLIC_KEY_LENGTH: usize = 32;
const ED25519_SIGNATURE_LENGTH: usize = 64;

/// A detached Ed25519 signature over the raw manifest bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureEnvelope {
    key_id: String,
    signature: [u8; ED25519_SIGNATURE_LENGTH],
}

#[derive(Debug)]
pub enum SignatureEnvelopeError {
    TooLarge,
    Malformed(serde_json::Error),
    UnsupportedFormatVersion(u32),
    UnsupportedAlgorithm(String),
    InvalidKeyId,
    InvalidSignatureEncoding(base64::DecodeError),
    InvalidSignatureLength,
}

/// One publisher key accepted by offline verification.
pub struct TrustedKey {
    key_id: String,
    publisher_id: PublisherId,
    verifying_key: VerifyingKey,
}

/// The set of publisher keys a verifier trusts.
pub struct TrustStore {
    keys: Vec<TrustedKey>,
}

#[derive(Debug)]
pub enum TrustStoreError {
    TooLarge,
    Malformed(serde_json::Error),
    UnsupportedFormatVersion(u32),
    Empty,
    InvalidKeyId,
    InvalidPublisherId,
    InvalidPublicKeyEncoding(base64::DecodeError),
    InvalidPublicKeyLength,
    InvalidPublicKey,
    DuplicateKeyId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSignature {
    format_version: u32,
    algorithm: String,
    key_id: String,
    signature: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTrustStore {
    format_version: u32,
    keys: Vec<RawTrustedKey>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTrustedKey {
    key_id: String,
    publisher_id: String,
    public_key: String,
}

impl SignatureEnvelope {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignatureEnvelopeError> {
        if bytes.len() > MAX_SIGNATURE_BYTES {
            return Err(SignatureEnvelopeError::TooLarge);
        }

        let raw: RawSignature =
            serde_json::from_slice(bytes).map_err(SignatureEnvelopeError::Malformed)?;

        if raw.format_version != FORMAT_VERSION {
            return Err(SignatureEnvelopeError::UnsupportedFormatVersion(
                raw.format_version,
            ));
        }

        if raw.algorithm != SIGNATURE_ALGORITHM {
            return Err(SignatureEnvelopeError::UnsupportedAlgorithm(raw.algorithm));
        }

        if !valid_key_id(&raw.key_id) {
            return Err(SignatureEnvelopeError::InvalidKeyId);
        }

        let decoded = URL_SAFE_NO_PAD
            .decode(&raw.signature)
            .map_err(SignatureEnvelopeError::InvalidSignatureEncoding)?;
        let signature: [u8; ED25519_SIGNATURE_LENGTH] = decoded
            .try_into()
            .map_err(|_| SignatureEnvelopeError::InvalidSignatureLength)?;

        Ok(Self {
            key_id: raw.key_id,
            signature,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn signature(&self) -> &[u8; ED25519_SIGNATURE_LENGTH] {
        &self.signature
    }
}

impl TrustStore {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TrustStoreError> {
        if bytes.len() > MAX_TRUST_STORE_BYTES {
            return Err(TrustStoreError::TooLarge);
        }

        let raw: RawTrustStore =
            serde_json::from_slice(bytes).map_err(TrustStoreError::Malformed)?;

        if raw.format_version != FORMAT_VERSION {
            return Err(TrustStoreError::UnsupportedFormatVersion(
                raw.format_version,
            ));
        }

        if raw.keys.is_empty() {
            return Err(TrustStoreError::Empty);
        }

        let mut key_ids = HashSet::new();
        let mut keys = Vec::with_capacity(raw.keys.len());

        for key in raw.keys {
            if !valid_key_id(&key.key_id) {
                return Err(TrustStoreError::InvalidKeyId);
            }
            if !key_ids.insert(key.key_id.clone()) {
                return Err(TrustStoreError::DuplicateKeyId);
            }

            let publisher_id = PublisherId::parse(key.publisher_id)
                .map_err(|_| TrustStoreError::InvalidPublisherId)?;

            let decoded = URL_SAFE_NO_PAD
                .decode(&key.public_key)
                .map_err(TrustStoreError::InvalidPublicKeyEncoding)?;
            let public_key: [u8; ED25519_PUBLIC_KEY_LENGTH] = decoded
                .try_into()
                .map_err(|_| TrustStoreError::InvalidPublicKeyLength)?;
            let verifying_key = VerifyingKey::from_bytes(&public_key)
                .map_err(|_| TrustStoreError::InvalidPublicKey)?;

            keys.push(TrustedKey {
                key_id: key.key_id,
                publisher_id,
                verifying_key,
            });
        }

        Ok(Self { keys })
    }

    pub fn verifying_key(&self, publisher_id: &PublisherId, key_id: &str) -> Option<&VerifyingKey> {
        self.keys
            .iter()
            .find(|key| key.key_id == key_id && &key.publisher_id == publisher_id)
            .map(|key| &key.verifying_key)
    }
}

impl TrustedKey {
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn publisher_id(&self) -> &PublisherId {
        &self.publisher_id
    }
}

pub(crate) fn valid_key_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_KEY_ID_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl fmt::Display for SignatureEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "package signature exceeds the size limit"),
            Self::Malformed(error) => write!(f, "package signature is malformed: {error}"),
            Self::UnsupportedFormatVersion(version) => {
                write!(f, "unsupported package signature format version {version}")
            }
            Self::UnsupportedAlgorithm(algorithm) => {
                write!(f, "unsupported package signature algorithm '{algorithm}'")
            }
            Self::InvalidKeyId => write!(f, "package signature has an invalid key id"),
            Self::InvalidSignatureEncoding(error) => {
                write!(f, "package signature is not valid base64: {error}")
            }
            Self::InvalidSignatureLength => write!(
                f,
                "package signature must decode to {ED25519_SIGNATURE_LENGTH} bytes"
            ),
        }
    }
}

impl Error for SignatureEnvelopeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(error) => Some(error),
            Self::InvalidSignatureEncoding(error) => Some(error),
            Self::TooLarge
            | Self::UnsupportedFormatVersion(_)
            | Self::UnsupportedAlgorithm(_)
            | Self::InvalidKeyId
            | Self::InvalidSignatureLength => None,
        }
    }
}

impl fmt::Display for TrustStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "trust store exceeds the size limit"),
            Self::Malformed(error) => write!(f, "trust store is malformed: {error}"),
            Self::UnsupportedFormatVersion(version) => {
                write!(f, "unsupported trust store format version {version}")
            }
            Self::Empty => write!(f, "trust store must contain at least one key"),
            Self::InvalidKeyId => write!(f, "trust store contains an invalid key id"),
            Self::InvalidPublisherId => {
                write!(f, "trust store contains an invalid publisher id")
            }
            Self::InvalidPublicKeyEncoding(error) => {
                write!(f, "trust store public key is not valid base64: {error}")
            }
            Self::InvalidPublicKeyLength => write!(
                f,
                "trust store public key must decode to {ED25519_PUBLIC_KEY_LENGTH} bytes"
            ),
            Self::InvalidPublicKey => write!(f, "trust store contains an invalid public key"),
            Self::DuplicateKeyId => write!(f, "trust store contains a duplicate key id"),
        }
    }
}

impl Error for TrustStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(error) => Some(error),
            Self::InvalidPublicKeyEncoding(error) => Some(error),
            Self::TooLarge
            | Self::UnsupportedFormatVersion(_)
            | Self::Empty
            | Self::InvalidKeyId
            | Self::InvalidPublisherId
            | Self::InvalidPublicKeyLength
            | Self::InvalidPublicKey
            | Self::DuplicateKeyId => None,
        }
    }
}
