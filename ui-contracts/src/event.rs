use serde::{Deserialize, Serialize};

pub const SHELL_EVENT_VERSION: u16 = 1;
pub const MAX_SHELL_EVENT_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShellEvent {
    SnapshotChanged {
        #[serde(rename = "eventVersion")]
        event_version: u16,
        revision: String,
    },
    SessionRevoked {
        #[serde(rename = "eventVersion")]
        event_version: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEventError {
    Invalid,
    TooLarge,
}

impl ShellEvent {
    pub fn snapshot_changed(revision: impl Into<String>) -> Result<Self, ShellEventError> {
        let revision = revision.into();
        if !valid_revision(&revision) {
            return Err(ShellEventError::Invalid);
        }
        Ok(Self::SnapshotChanged {
            event_version: SHELL_EVENT_VERSION,
            revision,
        })
    }

    pub fn session_revoked() -> Self {
        Self::SessionRevoked {
            event_version: SHELL_EVENT_VERSION,
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ShellEventError> {
        if bytes.len() > MAX_SHELL_EVENT_BYTES {
            return Err(ShellEventError::TooLarge);
        }
        let event: Self = serde_json::from_slice(bytes).map_err(|_| ShellEventError::Invalid)?;
        match &event {
            Self::SnapshotChanged {
                event_version,
                revision,
            } if *event_version == SHELL_EVENT_VERSION && valid_revision(revision) => Ok(event),
            Self::SessionRevoked { event_version } if *event_version == SHELL_EVENT_VERSION => {
                Ok(event)
            }
            _ => Err(ShellEventError::Invalid),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ShellEvent has only serializable fields")
    }
}

fn valid_revision(revision: &str) -> bool {
    !revision.is_empty()
        && revision.len() <= 128
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_bounded_revision_signal() {
        let event = ShellEvent::snapshot_changed("revision-42").unwrap();
        assert_eq!(ShellEvent::parse(event.to_json().as_bytes()), Ok(event));
    }

    #[test]
    fn refuses_unknown_fields_versions_and_oversized_events() {
        for payload in [
            r#"{"kind":"snapshot_changed","eventVersion":2,"revision":"r"}"#,
            r#"{"kind":"snapshot_changed","eventVersion":1,"revision":"../private"}"#,
            r#"{"kind":"session_revoked","eventVersion":1,"secret":"x"}"#,
        ] {
            assert_eq!(
                ShellEvent::parse(payload.as_bytes()),
                Err(ShellEventError::Invalid)
            );
        }
        assert_eq!(
            ShellEvent::parse(&vec![b'x'; MAX_SHELL_EVENT_BYTES + 1]),
            Err(ShellEventError::TooLarge)
        );
    }
}
