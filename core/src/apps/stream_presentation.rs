use std::error::Error;
use std::fmt;

use crate::{CapabilityId, RuntimeEntrypointId};

/// An engine implementation provides this capability; apps depend on the API,
/// not on the Selkies fork or a particular transport implementation.
pub const STREAMING_ENGINE_CAPABILITY: &str = "com.rumahl.streaming.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferredStreamSize {
    width: u16,
    height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamPresentation {
    app_entrypoint: RuntimeEntrypointId,
    preferred_size: Option<PreferredStreamSize>,
    preferred_frame_rate: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPresentationError {
    InvalidPreferredSize,
    InvalidPreferredFrameRate,
}

impl PreferredStreamSize {
    pub fn new(width: u16, height: u16) -> Result<Self, StreamPresentationError> {
        if !(320..=7680).contains(&width) || !(200..=4320).contains(&height) {
            return Err(StreamPresentationError::InvalidPreferredSize);
        }
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

impl StreamPresentation {
    pub fn new(
        app_entrypoint: RuntimeEntrypointId,
        preferred_size: Option<PreferredStreamSize>,
        preferred_frame_rate: Option<u8>,
    ) -> Result<Self, StreamPresentationError> {
        if preferred_frame_rate.is_some_and(|rate| !(1..=120).contains(&rate)) {
            return Err(StreamPresentationError::InvalidPreferredFrameRate);
        }
        Ok(Self {
            app_entrypoint,
            preferred_size,
            preferred_frame_rate,
        })
    }

    pub fn app_entrypoint(&self) -> &RuntimeEntrypointId {
        &self.app_entrypoint
    }

    pub fn preferred_size(&self) -> Option<PreferredStreamSize> {
        self.preferred_size
    }

    pub fn preferred_frame_rate(&self) -> Option<u8> {
        self.preferred_frame_rate
    }

    pub fn required_engine_capability(&self) -> CapabilityId {
        CapabilityId::parse(STREAMING_ENGINE_CAPABILITY)
            .expect("the versioned streaming capability is a valid identifier")
    }
}

impl fmt::Display for StreamPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPreferredSize => write!(formatter, "invalid preferred stream size"),
            Self::InvalidPreferredFrameRate => {
                write!(formatter, "invalid preferred stream frame rate")
            }
        }
    }
}

impl Error for StreamPresentationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_can_only_request_bounded_preferences() {
        assert_eq!(
            PreferredStreamSize::new(8192, 1080),
            Err(StreamPresentationError::InvalidPreferredSize)
        );
        assert_eq!(
            StreamPresentation::new(RuntimeEntrypointId::parse("main").unwrap(), None, Some(0)),
            Err(StreamPresentationError::InvalidPreferredFrameRate)
        );
        let preference = StreamPresentation::new(
            RuntimeEntrypointId::parse("main").unwrap(),
            Some(PreferredStreamSize::new(1920, 1080).unwrap()),
            Some(60),
        )
        .unwrap();
        assert_eq!(preference.preferred_frame_rate(), Some(60));
        assert_eq!(
            preference.required_engine_capability().as_str(),
            STREAMING_ENGINE_CAPABILITY
        );
    }
}
