use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretPurpose(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretPurposeError {
    Empty,
    TooLong,
    InvalidFormat,
}

impl SecretPurpose {
    pub const MAX_LENGTH: usize = 120;

    pub fn parse(value: impl Into<String>) -> Result<Self, SecretPurposeError> {
        let value = value.into();

        if value.is_empty() {
            return Err(SecretPurposeError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(SecretPurposeError::TooLong);
        }

        if value.split('.').any(|segment| {
            segment.is_empty()
                || segment.starts_with('-')
                || segment.ends_with('-')
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        }) {
            return Err(SecretPurposeError::InvalidFormat);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SecretPurpose {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SecretPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for SecretPurposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "secret purpose cannot be empty"),
            Self::TooLong => write!(
                f,
                "secret purpose cannot exceed {} bytes",
                SecretPurpose::MAX_LENGTH
            ),
            Self::InvalidFormat => write!(f, "secret purpose has an invalid format"),
        }
    }
}

impl Error for SecretPurposeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_namespaced_purpose() {
        assert_eq!(
            SecretPurpose::parse("rumahl.oidc.client-secret")
                .unwrap()
                .as_str(),
            "rumahl.oidc.client-secret"
        );
    }

    #[test]
    fn rejects_path_or_empty_segments() {
        for value in [
            "rumahl..secret",
            "Rumahl.secret",
            "../secret",
            "rumahl.secret/",
        ] {
            assert!(SecretPurpose::parse(value).is_err());
        }
    }
}
