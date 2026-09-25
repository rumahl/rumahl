//! Ed25519 ID-token signing. The private key is supplied by an OS-owned key
//! provider, never generated at request time or stored in an app snapshot.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use rumahl_core::UnixTimestamp;
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{OidcClientId, PairwiseSubject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcSigningKeyError {
    InvalidKeyId,
    InvalidIssuer,
    InvalidTime,
}

pub struct OidcSigningKey {
    key_id: String,
    private_seed: Zeroizing<[u8; 32]>,
}

impl OidcSigningKey {
    pub fn new(key_id: &str, private_seed: [u8; 32]) -> Result<Self, OidcSigningKeyError> {
        if key_id.is_empty()
            || key_id.len() > 64
            || !key_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(OidcSigningKeyError::InvalidKeyId);
        }
        Ok(Self {
            key_id: key_id.to_owned(),
            private_seed: Zeroizing::new(private_seed),
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_jwk(&self) -> Value {
        let key = SigningKey::from_bytes(&self.private_seed);
        json!({
            "kty": "OKP", "crv": "Ed25519", "alg": "EdDSA", "use": "sig",
            "kid": self.key_id,
            "x": URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_id_token(
        &self,
        issuer: &str,
        subject: &PairwiseSubject,
        audience: &OidcClientId,
        nonce: &str,
        auth_time: UnixTimestamp,
        issued_at: UnixTimestamp,
        expires_at: UnixTimestamp,
        name: Option<&str>,
    ) -> Result<String, OidcSigningKeyError> {
        let url = url::Url::parse(issuer).map_err(|_| OidcSigningKeyError::InvalidIssuer)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.origin().ascii_serialization() != issuer
        {
            return Err(OidcSigningKeyError::InvalidIssuer);
        }
        if auth_time > issued_at || issued_at >= expires_at {
            return Err(OidcSigningKeyError::InvalidTime);
        }
        let header = json!({"alg":"EdDSA","typ":"JWT","kid":self.key_id});
        let mut claims = json!({
            "iss": issuer, "sub": subject.value(), "aud": audience.as_str(),
            "iat": issued_at.as_seconds(), "exp": expires_at.as_seconds(),
            "auth_time": auth_time.as_seconds(), "nonce": nonce,
        });
        if let Some(name) = name {
            claims["name"] = json!(name);
        }
        let message = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string()),
        );
        let key = SigningKey::from_bytes(&self.private_seed);
        let signature = key.sign(message.as_bytes());
        Ok(format!(
            "{message}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    use rumahl_core::{InstallationId, UserId};

    #[test]
    fn published_jwk_verifies_signed_id_token() {
        let key = OidcSigningKey::new("key-1", [7; 32]).unwrap();
        let subject = PairwiseSubject::generate(UserId::new(), InstallationId::new()).unwrap();
        let audience = OidcClientId::generate().unwrap();
        let mut nonce_bytes = [0_u8; 32];
        getrandom::fill(&mut nonce_bytes).unwrap();
        let nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
        let jwt = key
            .sign_id_token(
                "https://rumahl.dev",
                &subject,
                &audience,
                &nonce,
                UnixTimestamp::from_seconds(100),
                UnixTimestamp::from_seconds(101),
                UnixTimestamp::from_seconds(161),
                Some("Alice"),
            )
            .unwrap();
        let parts = jwt.split('.').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3);
        let jwk = key.public_jwk();
        let public: [u8; 32] = URL_SAFE_NO_PAD
            .decode(jwk["x"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let verifier = VerifyingKey::from_bytes(&public).unwrap();
        let signature: [u8; 64] = URL_SAFE_NO_PAD
            .decode(parts[2])
            .unwrap()
            .try_into()
            .unwrap();
        verifier
            .verify(
                format!("{}.{}", parts[0], parts[1]).as_bytes(),
                &Signature::from_bytes(&signature),
            )
            .unwrap();
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["sub"], subject.value());
        assert_eq!(claims["aud"], audience.as_str());
        assert_eq!(claims["name"], "Alice");
        assert_eq!(claims["nonce"], nonce);
    }
}
