use std::error::Error;
use std::fmt;

use argon2::password_hash::{
    Error as PasswordHashingError, PasswordHasher, PasswordVerifier, phc::PasswordHash,
};
use argon2::{Algorithm, Argon2, Params, Version};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

const DEFAULT_MINIMUM_CHARACTERS: usize = 15;
const DEFAULT_MAXIMUM_CHARACTERS: usize = 1_024;

pub struct Password(Zeroizing<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordPolicy {
    minimum_characters: usize,
    maximum_characters: usize,
}

pub trait PasswordBlocklist {
    fn contains(&self, normalized_password: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordPolicyError {
    TooShort { minimum: usize },
    TooLong { maximum: usize },
    ContainsControlCharacter,
    Blocked,
    InvalidLengthRange,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PasswordHashRecord(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordHashRecordError {
    InvalidPhcString,
    UnsupportedAlgorithm,
    UnsupportedVersion,
    InvalidParameters,
    ParametersTooWeak,
    MissingSalt,
    MissingHash,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Argon2idPasswordEngine;

#[derive(Debug)]
pub enum PasswordEngineError {
    Hashing(PasswordHashingError),
    InvalidStoredHash(PasswordHashRecordError),
}

impl Password {
    pub fn prepare(candidate: String) -> Result<Self, PasswordPolicyError> {
        let normalized = candidate.nfc().collect::<String>();

        if normalized.chars().count() > DEFAULT_MAXIMUM_CHARACTERS {
            return Err(PasswordPolicyError::TooLong {
                maximum: DEFAULT_MAXIMUM_CHARACTERS,
            });
        }

        if normalized.chars().any(char::is_control) {
            return Err(PasswordPolicyError::ContainsControlCharacter);
        }

        Ok(Self(Zeroizing::new(normalized)))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Password").finish_non_exhaustive()
    }
}

impl PasswordPolicy {
    pub fn new(
        minimum_characters: usize,
        maximum_characters: usize,
    ) -> Result<Self, PasswordPolicyError> {
        if minimum_characters == 0
            || maximum_characters < minimum_characters
            || maximum_characters > DEFAULT_MAXIMUM_CHARACTERS
        {
            return Err(PasswordPolicyError::InvalidLengthRange);
        }

        Ok(Self {
            minimum_characters,
            maximum_characters,
        })
    }

    pub fn validate<B>(&self, password: &Password, blocklist: &B) -> Result<(), PasswordPolicyError>
    where
        B: PasswordBlocklist,
    {
        let character_count = password.expose().chars().count();

        if character_count < self.minimum_characters {
            return Err(PasswordPolicyError::TooShort {
                minimum: self.minimum_characters,
            });
        }

        if character_count > self.maximum_characters {
            return Err(PasswordPolicyError::TooLong {
                maximum: self.maximum_characters,
            });
        }

        if blocklist.contains(password.expose()) {
            return Err(PasswordPolicyError::Blocked);
        }

        Ok(())
    }

    pub fn minimum_characters(self) -> usize {
        self.minimum_characters
    }

    pub fn maximum_characters(self) -> usize {
        self.maximum_characters
    }
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            minimum_characters: DEFAULT_MINIMUM_CHARACTERS,
            maximum_characters: DEFAULT_MAXIMUM_CHARACTERS,
        }
    }
}

impl PasswordHashRecord {
    pub fn parse(encoded: String) -> Result<Self, PasswordHashRecordError> {
        let parsed =
            PasswordHash::new(&encoded).map_err(|_| PasswordHashRecordError::InvalidPhcString)?;

        if parsed.algorithm.as_str() != "argon2id" {
            return Err(PasswordHashRecordError::UnsupportedAlgorithm);
        }

        if parsed.version != Some(Version::V0x13.into()) {
            return Err(PasswordHashRecordError::UnsupportedVersion);
        }

        let parameters =
            Params::try_from(&parsed).map_err(|_| PasswordHashRecordError::InvalidParameters)?;

        if parameters.m_cost() < Params::DEFAULT_M_COST
            || parameters.t_cost() < Params::DEFAULT_T_COST
            || parameters.p_cost() < Params::DEFAULT_P_COST
            || parameters.output_len().unwrap_or_default() < Params::DEFAULT_OUTPUT_LEN
        {
            return Err(PasswordHashRecordError::ParametersTooWeak);
        }

        if parsed.salt.is_none() {
            return Err(PasswordHashRecordError::MissingSalt);
        }

        if parsed.hash.is_none() {
            return Err(PasswordHashRecordError::MissingHash);
        }

        Ok(Self(encoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PasswordHashRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordHashRecord").finish_non_exhaustive()
    }
}

impl Argon2idPasswordEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn hash(&self, password: &Password) -> Result<PasswordHashRecord, PasswordEngineError> {
        let encoded = self
            .engine()
            .hash_password(password.expose().as_bytes())
            .map_err(PasswordEngineError::Hashing)?
            .to_string();

        PasswordHashRecord::parse(encoded).map_err(PasswordEngineError::InvalidStoredHash)
    }

    pub fn verify(
        &self,
        password: &Password,
        record: &PasswordHashRecord,
    ) -> Result<bool, PasswordEngineError> {
        let parsed = PasswordHash::new(record.as_str()).map_err(|_| {
            PasswordEngineError::InvalidStoredHash(PasswordHashRecordError::InvalidPhcString)
        })?;

        match self
            .engine()
            .verify_password(password.expose().as_bytes(), &parsed)
        {
            Ok(()) => Ok(true),
            Err(PasswordHashingError::PasswordInvalid) => Ok(false),
            Err(error) => Err(PasswordEngineError::Hashing(error)),
        }
    }

    fn engine(&self) -> Argon2<'static> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::DEFAULT)
    }
}

impl fmt::Display for PasswordPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { minimum } => {
                write!(f, "password must contain at least {minimum} characters")
            }
            Self::TooLong { maximum } => {
                write!(f, "password must contain at most {maximum} characters")
            }
            Self::ContainsControlCharacter => {
                write!(f, "password must not contain control characters")
            }
            Self::Blocked => write!(f, "password appears in the configured blocklist"),
            Self::InvalidLengthRange => write!(f, "password policy length range is invalid"),
        }
    }
}

impl Error for PasswordPolicyError {}

impl fmt::Display for PasswordHashRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhcString => write!(f, "password verifier is not a valid PHC string"),
            Self::UnsupportedAlgorithm => {
                write!(f, "password verifier does not use Argon2id")
            }
            Self::UnsupportedVersion => {
                write!(f, "password verifier uses an unsupported Argon2 version")
            }
            Self::InvalidParameters => write!(f, "password verifier parameters are invalid"),
            Self::ParametersTooWeak => {
                write!(
                    f,
                    "password verifier parameters are below the security policy"
                )
            }
            Self::MissingSalt => write!(f, "password verifier does not contain a salt"),
            Self::MissingHash => write!(f, "password verifier does not contain a hash"),
        }
    }
}

impl Error for PasswordHashRecordError {}

impl fmt::Display for PasswordEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hashing(_) => write!(f, "Argon2id password operation failed"),
            Self::InvalidStoredHash(_) => write!(f, "stored password verifier is invalid"),
        }
    }
}

impl Error for PasswordEngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Hashing(error) => Some(error),
            Self::InvalidStoredHash(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBlocklist;

    impl PasswordBlocklist for TestBlocklist {
        fn contains(&self, normalized_password: &str) -> bool {
            normalized_password == "this password is blocked"
        }
    }

    #[test]
    fn default_policy_accepts_long_unicode_passphrase_without_composition_rules() {
        let password = Password::prepare("ein langes sicheres passwort 🏠".to_owned()).unwrap();

        assert!(
            PasswordPolicy::default()
                .validate(&password, &TestBlocklist)
                .is_ok()
        );
    }

    #[test]
    fn normalizes_unicode_before_hashing() {
        let composed = Password::prepare("12345678901234é".to_owned()).unwrap();
        let decomposed = Password::prepare("12345678901234e\u{301}".to_owned()).unwrap();
        let engine = Argon2idPasswordEngine::new();
        let hash = engine.hash(&composed).unwrap();

        assert!(engine.verify(&decomposed, &hash).unwrap());
    }

    #[test]
    fn policy_rejects_short_and_blocked_passwords() {
        let short = Password::prepare("too short".to_owned()).unwrap();
        let blocked = Password::prepare("this password is blocked".to_owned()).unwrap();
        let policy = PasswordPolicy::default();

        assert!(matches!(
            policy.validate(&short, &TestBlocklist),
            Err(PasswordPolicyError::TooShort { minimum: 15 })
        ));
        assert_eq!(
            policy.validate(&blocked, &TestBlocklist),
            Err(PasswordPolicyError::Blocked)
        );
    }

    #[test]
    fn hashes_with_argon2id_and_unique_salts() {
        let password = Password::prepare("correct horse battery staple".to_owned()).unwrap();
        let engine = Argon2idPasswordEngine::new();
        let first = engine.hash(&password).unwrap();
        let second = engine.hash(&password).unwrap();

        assert!(
            first
                .as_str()
                .starts_with("$argon2id$v=19$m=19456,t=2,p=1$")
        );
        assert_ne!(first.as_str(), second.as_str());
        assert!(engine.verify(&password, &first).unwrap());
    }

    #[test]
    fn debug_output_does_not_expose_password_or_hash() {
        let password = Password::prepare("correct horse battery staple".to_owned()).unwrap();
        let hash = Argon2idPasswordEngine::new().hash(&password).unwrap();

        assert_eq!(format!("{password:?}"), "Password { .. }");
        assert_eq!(format!("{hash:?}"), "PasswordHashRecord { .. }");
    }

    #[test]
    fn rejects_argon2id_verifier_below_policy_parameters() {
        let password = Password::prepare("correct horse battery staple".to_owned()).unwrap();
        let weak_engine = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(4_096, 1, 1, Some(16)).unwrap(),
        );
        let weak_hash = weak_engine
            .hash_password(password.expose().as_bytes())
            .unwrap()
            .to_string();

        assert_eq!(
            PasswordHashRecord::parse(weak_hash).unwrap_err(),
            PasswordHashRecordError::ParametersTooWeak
        );
    }
}
