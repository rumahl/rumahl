//! Authenticated encryption for sensitive database columns.
//!
//! A value is sealed with AES-256-GCM under the OS encryption key and bound to a
//! caller-supplied *context* with the associated data. The context should name
//! the database, table, column and row identity so a ciphertext cannot be moved
//! to another row or column undetected. The envelope records the key id so
//! historical keys stay usable for reads and rotation.

use std::error::Error;
use std::fmt;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use zeroize::Zeroizing;

use crate::secret_store::{
    SecretEncryptionKey, SecretEncryptionKeyId, SecretEncryptionKeyProvider,
};

const NONCE_LENGTH: usize = 12;
const CONTEXT_VERSION: &[u8] = b"rumahl-field-v1";
const ENVELOPE_MAGIC: &[u8; 4] = b"RFE1";

type BoxedError = Box<dyn Error + Send + Sync + 'static>;

/// Object-safe key source for field encryption, so providers can be stored
/// behind an `Arc<dyn FieldKeyProvider>` (for example the personal-files app).
pub trait FieldKeyProvider: Send + Sync {
    fn active_key(&self) -> Result<SecretEncryptionKey, BoxedError>;

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, BoxedError>;
}

impl<K> FieldKeyProvider for K
where
    K: SecretEncryptionKeyProvider + Send + Sync,
{
    fn active_key(&self) -> Result<SecretEncryptionKey, BoxedError> {
        <K as SecretEncryptionKeyProvider>::active_key(self)
            .map_err(|error| Box::new(error) as BoxedError)
    }

    fn key_by_id(
        &self,
        id: &SecretEncryptionKeyId,
    ) -> Result<Option<SecretEncryptionKey>, BoxedError> {
        <K as SecretEncryptionKeyProvider>::key_by_id(self, id)
            .map_err(|error| Box::new(error) as BoxedError)
    }
}

/// A sealed column value: key id, nonce and ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEnvelope {
    pub key_id: SecretEncryptionKeyId,
    pub nonce: [u8; NONCE_LENGTH],
    pub ciphertext: Vec<u8>,
}

#[derive(Debug)]
pub enum FieldCipherError {
    KeyProvider(BoxedError),
    InvalidKeyLength,
    Random(getrandom::Error),
    EncryptionFailed,
    AuthenticationFailed,
    MissingKey(String),
    InvalidNonceLength(usize),
    InvalidEnvelope,
}

/// Encrypts `plaintext` with the provider's active key under `context`.
pub fn seal_field(
    provider: &dyn FieldKeyProvider,
    context: &[u8],
    plaintext: &[u8],
) -> Result<FieldEnvelope, FieldCipherError> {
    let key = provider
        .active_key()
        .map_err(FieldCipherError::KeyProvider)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|_| FieldCipherError::InvalidKeyLength)?;
    let mut nonce = [0_u8; NONCE_LENGTH];
    getrandom::fill(&mut nonce).map_err(FieldCipherError::Random)?;
    let nonce_value = Nonce::try_from(&nonce[..])
        .map_err(|_| FieldCipherError::InvalidNonceLength(nonce.len()))?;
    let ciphertext = cipher
        .encrypt(
            &nonce_value,
            Payload {
                msg: plaintext,
                aad: &associated_data(context),
            },
        )
        .map_err(|_| FieldCipherError::EncryptionFailed)?;
    Ok(FieldEnvelope {
        key_id: key.id().clone(),
        nonce,
        ciphertext,
    })
}

/// Decrypts a sealed value, looking up the key recorded in the envelope.
pub fn open_field(
    provider: &dyn FieldKeyProvider,
    context: &[u8],
    envelope: &FieldEnvelope,
) -> Result<Zeroizing<Vec<u8>>, FieldCipherError> {
    let key = provider
        .key_by_id(&envelope.key_id)
        .map_err(FieldCipherError::KeyProvider)?
        .ok_or_else(|| FieldCipherError::MissingKey(envelope.key_id.to_string()))?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|_| FieldCipherError::InvalidKeyLength)?;
    let nonce = Nonce::try_from(&envelope.nonce[..])
        .map_err(|_| FieldCipherError::InvalidNonceLength(envelope.nonce.len()))?;
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &envelope.ciphertext,
                aad: &associated_data(context),
            },
        )
        .map_err(|_| FieldCipherError::AuthenticationFailed)?;
    Ok(Zeroizing::new(plaintext))
}

impl FieldEnvelope {
    /// Serializes the envelope into one bounded blob for a single column.
    pub fn encode(&self) -> Vec<u8> {
        let key_id = self.key_id.as_str().as_bytes();
        let mut bytes = Vec::with_capacity(
            ENVELOPE_MAGIC.len() + 2 + key_id.len() + NONCE_LENGTH + self.ciphertext.len(),
        );
        bytes.extend_from_slice(ENVELOPE_MAGIC);
        bytes.extend_from_slice(&(key_id.len() as u16).to_be_bytes());
        bytes.extend_from_slice(key_id);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FieldCipherError> {
        if bytes.len() < ENVELOPE_MAGIC.len() + 2 + NONCE_LENGTH
            || &bytes[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC
        {
            return Err(FieldCipherError::InvalidEnvelope);
        }
        let key_id_length = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let key_id_start: usize = 6;
        let nonce_start = key_id_start
            .checked_add(key_id_length)
            .ok_or(FieldCipherError::InvalidEnvelope)?;
        let ciphertext_start = nonce_start
            .checked_add(NONCE_LENGTH)
            .ok_or(FieldCipherError::InvalidEnvelope)?;
        if ciphertext_start > bytes.len() || key_id_length == 0 {
            return Err(FieldCipherError::InvalidEnvelope);
        }
        let key_id = std::str::from_utf8(&bytes[key_id_start..nonce_start])
            .ok()
            .and_then(|value| SecretEncryptionKeyId::parse(value).ok())
            .ok_or(FieldCipherError::InvalidEnvelope)?;
        let mut nonce = [0_u8; NONCE_LENGTH];
        nonce.copy_from_slice(&bytes[nonce_start..ciphertext_start]);
        Ok(Self {
            key_id,
            nonce,
            ciphertext: bytes[ciphertext_start..].to_vec(),
        })
    }
}

fn associated_data(context: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(CONTEXT_VERSION.len() + 4 + context.len());
    data.extend_from_slice(CONTEXT_VERSION);
    data.extend_from_slice(&(context.len() as u32).to_be_bytes());
    data.extend_from_slice(context);
    data
}

impl fmt::Display for FieldCipherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyProvider(error) => write!(f, "field encryption key provider failed: {error}"),
            Self::InvalidKeyLength => write!(f, "field encryption key has an invalid length"),
            Self::Random(error) => write!(f, "field encryption nonce generation failed: {error}"),
            Self::EncryptionFailed => write!(f, "field encryption failed"),
            Self::AuthenticationFailed => write!(f, "field decryption authentication failed"),
            Self::MissingKey(id) => write!(f, "field encryption key '{id}' is unavailable"),
            Self::InvalidNonceLength(length) => {
                write!(f, "field encryption nonce has invalid length {length}")
            }
            Self::InvalidEnvelope => write!(f, "field encryption envelope is invalid"),
        }
    }
}

impl Error for FieldCipherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::KeyProvider(error) => Some(error.as_ref()),
            Self::Random(error) => Some(error),
            Self::InvalidKeyLength
            | Self::EncryptionFailed
            | Self::AuthenticationFailed
            | Self::MissingKey(_)
            | Self::InvalidNonceLength(_)
            | Self::InvalidEnvelope => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("key error")
        }
    }
    impl Error for TestError {}

    struct Provider;

    impl SecretEncryptionKeyProvider for Provider {
        type Error = TestError;

        fn active_key(&self) -> Result<SecretEncryptionKey, Self::Error> {
            Ok(SecretEncryptionKey::new(
                SecretEncryptionKeyId::parse("key-1").unwrap(),
                [3_u8; 32],
            ))
        }

        fn key_by_id(
            &self,
            id: &SecretEncryptionKeyId,
        ) -> Result<Option<SecretEncryptionKey>, Self::Error> {
            Ok((id.as_str() == "key-1").then(|| {
                SecretEncryptionKey::new(SecretEncryptionKeyId::parse("key-1").unwrap(), [3_u8; 32])
            }))
        }
    }

    #[test]
    fn round_trips_and_hides_the_plaintext() {
        let envelope = seal_field(&Provider, b"files.content:row-1", b"secret bytes").unwrap();
        assert!(
            !envelope
                .ciphertext
                .windows(b"secret bytes".len())
                .any(|window| window == b"secret bytes")
        );
        let restored = open_field(&Provider, b"files.content:row-1", &envelope).unwrap();
        assert_eq!(&restored[..], b"secret bytes");
    }

    #[test]
    fn rejects_a_different_context() {
        let envelope = seal_field(&Provider, b"files.content:row-1", b"secret bytes").unwrap();
        assert!(open_field(&Provider, b"files.content:row-2", &envelope).is_err());
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let mut envelope = seal_field(&Provider, b"ctx", b"secret bytes").unwrap();
        envelope.ciphertext[0] ^= 0xff;
        assert!(open_field(&Provider, b"ctx", &envelope).is_err());
    }

    #[test]
    fn envelope_round_trips_through_bytes() {
        let envelope = seal_field(&Provider, b"ctx", b"secret bytes").unwrap();
        let encoded = envelope.encode();
        assert_eq!(FieldEnvelope::decode(&encoded).unwrap(), envelope);
        assert!(FieldEnvelope::decode(b"not an envelope").is_err());
    }
}
