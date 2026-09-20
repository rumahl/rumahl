use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackagePath(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePathError {
    Empty,
    TooLong,
    Absolute,
    EmptySegment,
    CurrentSegment,
    ParentSegment,
    InvalidCharacter(char),
}

impl PackagePath {
    pub const MAX_LENGTH: usize = 1024;

    pub fn parse(value: impl Into<String>) -> Result<Self, PackagePathError> {
        let value = value.into();

        if value.is_empty() {
            return Err(PackagePathError::Empty);
        }

        if value.len() > Self::MAX_LENGTH {
            return Err(PackagePathError::TooLong);
        }

        if value.starts_with('/') {
            return Err(PackagePathError::Absolute);
        }

        for segment in value.split('/') {
            if segment.is_empty() {
                return Err(PackagePathError::EmptySegment);
            }

            if segment == "." {
                return Err(PackagePathError::CurrentSegment);
            }

            if segment == ".." {
                return Err(PackagePathError::ParentSegment);
            }

            for character in segment.chars() {
                if !character.is_ascii_alphanumeric()
                    && character != '-'
                    && character != '_'
                    && character != '.'
                {
                    return Err(PackagePathError::InvalidCharacter(character));
                }
            }
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackagePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for PackagePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for PackagePath {
    type Error = PackagePathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for PackagePath {
    type Error = PackagePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for PackagePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                write!(f, "package path must not be empty")
            }

            Self::TooLong => {
                write!(
                    f,
                    "package path must not exceed {} bytes",
                    PackagePath::MAX_LENGTH
                )
            }

            Self::Absolute => {
                write!(f, "package path must be relative")
            }

            Self::EmptySegment => {
                write!(f, "package path must not contain empty segments")
            }

            Self::CurrentSegment => {
                write!(
                    f,
                    "package path must not contain current-directory segments"
                )
            }

            Self::ParentSegment => {
                write!(f, "package path must not contain parent-directory segments")
            }

            Self::InvalidCharacter(character) => {
                write!(f, "package path contains invalid character '{character}'")
            }
        }
    }
}

impl Error for PackagePathError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_package_relative_paths() {
        assert_eq!(
            PackagePath::parse("frontend/index.html").unwrap().as_str(),
            "frontend/index.html"
        );

        assert!(PackagePath::parse("runtime/server.oci").is_ok());
    }

    #[test]
    fn rejects_host_and_traversal_paths() {
        assert_eq!(
            PackagePath::parse("/etc/passwd").unwrap_err(),
            PackagePathError::Absolute
        );

        assert_eq!(
            PackagePath::parse("C:\\Windows\\system32").unwrap_err(),
            PackagePathError::InvalidCharacter(':')
        );

        assert_eq!(
            PackagePath::parse("runtime/../secrets").unwrap_err(),
            PackagePathError::ParentSegment
        );

        assert_eq!(
            PackagePath::parse("frontend//index.html").unwrap_err(),
            PackagePathError::EmptySegment
        );
    }
}
