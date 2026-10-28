use std::error::Error;
use std::fmt;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixTimestamp(u64);

#[derive(Debug)]
pub enum UnixTimestampError {
    BeforeEpoch(SystemTimeError),
}

impl UnixTimestamp {
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub fn now() -> Result<Self, UnixTimestampError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| Self(duration.as_secs()))
            .map_err(UnixTimestampError::BeforeEpoch)
    }

    pub const fn as_seconds(self) -> u64 {
        self.0
    }
}

impl fmt::Display for UnixTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for UnixTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeEpoch(error) => write!(f, "system time is before the Unix epoch: {error}"),
        }
    }
}

impl Error for UnixTimestampError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BeforeEpoch(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unix_seconds() {
        let timestamp = UnixTimestamp::from_seconds(1_700_000_000);

        assert_eq!(timestamp.as_seconds(), 1_700_000_000);
    }

    #[test]
    fn current_timestamp_is_after_unix_epoch() {
        assert!(UnixTimestamp::now().unwrap().as_seconds() > 0);
    }
}
