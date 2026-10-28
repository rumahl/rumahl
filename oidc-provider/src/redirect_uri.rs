use std::error::Error;
use std::fmt;

use rumahl_core::OidcCallbackPath;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcRedirectUri(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidcRedirectUriError {
    InvalidUrl(url::ParseError),
    HttpsRequired,
    HostRequired,
    CredentialsNotAllowed,
    OriginPathNotAllowed,
    QueryNotAllowed,
    FragmentNotAllowed,
    NonCanonical,
}

impl OidcRedirectUri {
    pub fn from_platform_origin(
        origin: &str,
        callback_path: &OidcCallbackPath,
    ) -> Result<Self, OidcRedirectUriError> {
        let mut parsed = validate_origin(origin)?;
        parsed.set_path(callback_path.as_str());
        let value = parsed.to_string();

        Self::parse(value)
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, OidcRedirectUriError> {
        let value = value.into();
        let parsed = Url::parse(&value).map_err(OidcRedirectUriError::InvalidUrl)?;

        validate_https_url(&parsed)?;

        if parsed.query().is_some() {
            return Err(OidcRedirectUriError::QueryNotAllowed);
        }

        if parsed.fragment().is_some() {
            return Err(OidcRedirectUriError::FragmentNotAllowed);
        }

        if parsed.as_str() != value {
            return Err(OidcRedirectUriError::NonCanonical);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn exactly_matches(&self, candidate: &str) -> bool {
        self.0 == candidate
    }
}

impl fmt::Display for OidcRedirectUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_origin(origin: &str) -> Result<Url, OidcRedirectUriError> {
    let parsed = Url::parse(origin).map_err(OidcRedirectUriError::InvalidUrl)?;
    validate_https_url(&parsed)?;

    if parsed.path() != "/" {
        return Err(OidcRedirectUriError::OriginPathNotAllowed);
    }

    if parsed.query().is_some() {
        return Err(OidcRedirectUriError::QueryNotAllowed);
    }

    if parsed.fragment().is_some() {
        return Err(OidcRedirectUriError::FragmentNotAllowed);
    }

    Ok(parsed)
}

fn validate_https_url(parsed: &Url) -> Result<(), OidcRedirectUriError> {
    if parsed.scheme() != "https" {
        return Err(OidcRedirectUriError::HttpsRequired);
    }

    if parsed.host_str().is_none() {
        return Err(OidcRedirectUriError::HostRequired);
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(OidcRedirectUriError::CredentialsNotAllowed);
    }

    Ok(())
}

impl fmt::Display for OidcRedirectUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(_) => write!(f, "OIDC redirect URI is not a valid URL"),
            Self::HttpsRequired => write!(f, "OIDC redirect URI must use HTTPS"),
            Self::HostRequired => write!(f, "OIDC redirect URI must contain a host"),
            Self::CredentialsNotAllowed => {
                write!(f, "OIDC redirect URI must not contain URL credentials")
            }
            Self::OriginPathNotAllowed => {
                write!(f, "platform runtime origin must not contain a path")
            }
            Self::QueryNotAllowed => write!(f, "OIDC redirect URI must not contain a query"),
            Self::FragmentNotAllowed => {
                write!(f, "OIDC redirect URI must not contain a fragment")
            }
            Self::NonCanonical => write!(f, "OIDC redirect URI is not canonical"),
        }
    }
}

impl Error for OidcRedirectUriError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUrl(error) => Some(error),
            Self::HttpsRequired
            | Self::HostRequired
            | Self::CredentialsNotAllowed
            | Self::OriginPathNotAllowed
            | Self::QueryNotAllowed
            | Self::FragmentNotAllowed
            | Self::NonCanonical => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_callback_only_against_platform_https_origin() {
        let callback = OidcCallbackPath::parse("/oidc/callback").unwrap();
        let redirect =
            OidcRedirectUri::from_platform_origin("https://notes.rumahl.local/", &callback)
                .unwrap();

        assert_eq!(
            redirect.as_str(),
            "https://notes.rumahl.local/oidc/callback"
        );
    }

    #[test]
    fn rejects_manifest_independent_unsafe_origins() {
        let callback = OidcCallbackPath::parse("/oidc/callback").unwrap();

        assert_eq!(
            OidcRedirectUri::from_platform_origin("http://notes.local/", &callback).unwrap_err(),
            OidcRedirectUriError::HttpsRequired
        );
        assert_eq!(
            OidcRedirectUri::from_platform_origin("https://evil@notes.local/", &callback)
                .unwrap_err(),
            OidcRedirectUriError::CredentialsNotAllowed
        );
        assert_eq!(
            OidcRedirectUri::from_platform_origin("https://notes.local/base", &callback)
                .unwrap_err(),
            OidcRedirectUriError::OriginPathNotAllowed
        );
    }

    #[test]
    fn redirect_matching_is_exact_string_matching() {
        let redirect = OidcRedirectUri::parse("https://notes.local/oidc/callback").unwrap();

        assert!(redirect.exactly_matches("https://notes.local/oidc/callback"));
        assert!(!redirect.exactly_matches("https://notes.local/oidc/callback/"));
        assert!(!redirect.exactly_matches("https://NOTES.local/oidc/callback"));
    }
}
