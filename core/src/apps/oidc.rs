use std::error::Error;
use std::fmt;

use crate::RuntimeEntrypointId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidcClientType {
    Public,
    Confidential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OidcScope {
    OpenId,
    Profile,
    Email,
    OfflineAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcCallbackPath(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcCallbackPathError {
    Empty,
    TooLong,
    MustBeAbsolute,
    AuthorityLikePath,
    EmptySegment,
    DotSegment,
    QueryOrFragment,
    InvalidCharacter(char),
    PercentEncodingNotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcClientDeclaration {
    client_type: OidcClientType,
    callback_entrypoint: RuntimeEntrypointId,
    callback_path: OidcCallbackPath,
    scopes: Vec<OidcScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcClientDeclarationError {
    MissingOpenIdScope,
    DuplicateScope(OidcScope),
    TooManyScopes,
}

impl OidcCallbackPath {
    pub const MAX_LENGTH: usize = 2_048;

    pub fn parse(value: impl Into<String>) -> Result<Self, OidcCallbackPathError> {
        let value = value.into();

        if value.is_empty() {
            return Err(OidcCallbackPathError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(OidcCallbackPathError::TooLong);
        }

        if !value.starts_with('/') {
            return Err(OidcCallbackPathError::MustBeAbsolute);
        }

        if value.starts_with("//") {
            return Err(OidcCallbackPathError::AuthorityLikePath);
        }

        if value.contains(['?', '#']) {
            return Err(OidcCallbackPathError::QueryOrFragment);
        }

        for segment in value[1..].split('/') {
            if segment.is_empty() {
                return Err(OidcCallbackPathError::EmptySegment);
            }

            validate_segment(segment)?;

            if matches!(segment, "." | "..") {
                return Err(OidcCallbackPathError::DotSegment);
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for OidcCallbackPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for OidcCallbackPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl OidcClientDeclaration {
    pub const MAX_SCOPES: usize = 8;

    pub fn new(
        client_type: OidcClientType,
        callback_entrypoint: RuntimeEntrypointId,
        callback_path: OidcCallbackPath,
        scopes: Vec<OidcScope>,
    ) -> Result<Self, OidcClientDeclarationError> {
        if scopes.len() > Self::MAX_SCOPES {
            return Err(OidcClientDeclarationError::TooManyScopes);
        }

        if !scopes.contains(&OidcScope::OpenId) {
            return Err(OidcClientDeclarationError::MissingOpenIdScope);
        }

        for (index, scope) in scopes.iter().enumerate() {
            if scopes[..index].contains(scope) {
                return Err(OidcClientDeclarationError::DuplicateScope(*scope));
            }
        }

        Ok(Self {
            client_type,
            callback_entrypoint,
            callback_path,
            scopes,
        })
    }

    pub fn client_type(&self) -> OidcClientType {
        self.client_type
    }

    pub fn callback_entrypoint(&self) -> &RuntimeEntrypointId {
        &self.callback_entrypoint
    }

    pub fn callback_path(&self) -> &OidcCallbackPath {
        &self.callback_path
    }

    pub fn scopes(&self) -> &[OidcScope] {
        &self.scopes
    }
}

impl OidcScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenId => "openid",
            Self::Profile => "profile",
            Self::Email => "email",
            Self::OfflineAccess => "offline_access",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "openid" => Some(Self::OpenId),
            "profile" => Some(Self::Profile),
            "email" => Some(Self::Email),
            "offline_access" => Some(Self::OfflineAccess),
            _ => None,
        }
    }
}

fn validate_segment(segment: &str) -> Result<(), OidcCallbackPathError> {
    let bytes = segment.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'%' {
            return Err(OidcCallbackPathError::PercentEncodingNotAllowed);
        }

        let character = char::from(byte);
        if !byte.is_ascii()
            || !(byte.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~'))
        {
            return Err(OidcCallbackPathError::InvalidCharacter(character));
        }

        index += 1;
    }

    Ok(())
}

impl fmt::Display for OidcCallbackPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "OIDC callback path must not be empty"),
            Self::TooLong => write!(f, "OIDC callback path is too long"),
            Self::MustBeAbsolute => write!(f, "OIDC callback path must start with '/'"),
            Self::AuthorityLikePath => write!(f, "OIDC callback path must not start with '//'"),
            Self::EmptySegment => write!(f, "OIDC callback path must not contain empty segments"),
            Self::DotSegment => write!(f, "OIDC callback path must not contain dot segments"),
            Self::QueryOrFragment => {
                write!(f, "OIDC callback path must not contain a query or fragment")
            }
            Self::InvalidCharacter(character) => {
                write!(
                    f,
                    "OIDC callback path contains invalid character '{character}'"
                )
            }
            Self::PercentEncodingNotAllowed => {
                write!(f, "OIDC callback path must not contain percent encoding")
            }
        }
    }
}

impl Error for OidcCallbackPathError {}

impl fmt::Display for OidcClientDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOpenIdScope => write!(f, "OIDC declaration requires the openid scope"),
            Self::DuplicateScope(scope) => {
                write!(
                    f,
                    "OIDC declaration contains duplicate scope '{}'",
                    scope.as_str()
                )
            }
            Self::TooManyScopes => write!(f, "OIDC declaration contains too many scopes"),
        }
    }
}

impl Error for OidcClientDeclarationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_strict_callback_path() {
        assert_eq!(
            OidcCallbackPath::parse("/apps/oidc/callback")
                .unwrap()
                .as_str(),
            "/apps/oidc/callback"
        );
    }

    #[test]
    fn rejects_authority_query_dot_and_encoded_separator_paths() {
        assert_eq!(
            OidcCallbackPath::parse("//evil.example/callback").unwrap_err(),
            OidcCallbackPathError::AuthorityLikePath
        );
        assert_eq!(
            OidcCallbackPath::parse("/callback?next=evil").unwrap_err(),
            OidcCallbackPathError::QueryOrFragment
        );
        assert_eq!(
            OidcCallbackPath::parse("/callback/../escape").unwrap_err(),
            OidcCallbackPathError::DotSegment
        );
        assert_eq!(
            OidcCallbackPath::parse("/callback%2Fescape").unwrap_err(),
            OidcCallbackPathError::PercentEncodingNotAllowed
        );
    }

    #[test]
    fn declaration_requires_openid_and_unique_scopes() {
        let entrypoint = RuntimeEntrypointId::parse("main").unwrap();
        let path = OidcCallbackPath::parse("/oidc/callback").unwrap();

        assert_eq!(
            OidcClientDeclaration::new(
                OidcClientType::Public,
                entrypoint.clone(),
                path.clone(),
                vec![OidcScope::Profile],
            )
            .unwrap_err(),
            OidcClientDeclarationError::MissingOpenIdScope
        );
        assert_eq!(
            OidcClientDeclaration::new(
                OidcClientType::Public,
                entrypoint,
                path,
                vec![OidcScope::OpenId, OidcScope::OpenId],
            )
            .unwrap_err(),
            OidcClientDeclarationError::DuplicateScope(OidcScope::OpenId)
        );
    }
}
