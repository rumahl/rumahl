use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppVersionError {
    InvalidFormat,
    InvalidNumber,
    LeadingZero,
}

impl AppVersion {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppVersionError> {
        let mut parts = value.split('.');

        let major = parts.next().ok_or(AppVersionError::InvalidFormat)?;

        let minor = parts.next().ok_or(AppVersionError::InvalidFormat)?;

        let patch = parts.next().ok_or(AppVersionError::InvalidFormat)?;

        if parts.next().is_some() {
            return Err(AppVersionError::InvalidFormat);
        }

        let major = Self::parse_component(major)?;

        let minor = Self::parse_component(minor)?;

        let patch = Self::parse_component(patch)?;

        Ok(Self::new(major, minor, patch))
    }

    fn parse_component(value: &str) -> Result<u64, AppVersionError> {
        if value.is_empty() {
            return Err(AppVersionError::InvalidFormat);
        }
        let number = value
            .parse::<u64>()
            .map_err(|_| AppVersionError::InvalidNumber)?;

        //TODO: Consider whether leading zeros should be allowed after all
        if value.len() > 1 && value.starts_with('0') {
            return Err(AppVersionError::LeadingZero);
        }

        Ok(number)
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl TryFrom<&str> for AppVersion {
    type Error = AppVersionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for AppVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => {
                write!(f, "app version must use major.minor.patch format")
            }

            Self::InvalidNumber => {
                write!(f, "app version components must be non-negative integers")
            }

            Self::LeadingZero => {
                write!(f, "app version components cannot contain leading zeros")
            }
        }
    }
}

impl Error for AppVersionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_app_version() {
        let version = AppVersion::new(1, 4, 2);

        assert_eq!(version.major(), 1);

        assert_eq!(version.minor(), 4);

        assert_eq!(version.patch(), 2);
    }

    #[test]
    fn parses_app_version() {
        let version = AppVersion::parse("1.4.2").unwrap();

        assert_eq!(version, AppVersion::new(1, 4, 2,));
    }

    #[test]
    fn formats_app_version() {
        let version = AppVersion::new(2, 17, 4);

        assert_eq!(version.to_string(), "2.17.4");
    }

    #[test]
    fn accepts_zero_version() {
        let version = AppVersion::parse("0.0.0").unwrap();

        assert_eq!(version, AppVersion::new(0, 0, 0,));
    }

    #[test]
    fn rejects_missing_patch_version() {
        assert_eq!(
            AppVersion::parse("1.2").unwrap_err(),
            AppVersionError::InvalidFormat
        );
    }

    #[test]
    fn rejects_extra_version_component() {
        assert_eq!(
            AppVersion::parse("1.2.3.4").unwrap_err(),
            AppVersionError::InvalidFormat
        );
    }

    #[test]
    fn rejects_non_numeric_component() {
        assert_eq!(
            AppVersion::parse("1.x.3").unwrap_err(),
            AppVersionError::InvalidNumber
        );
    }

    #[test]
    fn rejects_leading_zero() {
        assert_eq!(
            AppVersion::parse("01.2.3").unwrap_err(),
            AppVersionError::LeadingZero
        );
    }

    #[test]
    fn rejects_pre_release_version_for_now() {
        assert_eq!(
            AppVersion::parse("1.0.0-beta").unwrap_err(),
            AppVersionError::InvalidNumber
        );
    }
}
