//! Append-only security audit trail.
//!
//! The audit log records *what happened*, never secret material: no passwords,
//! session tokens, credentials, authorization codes or database queries. Targets
//! are bounded, non-secret identifiers. The persistence layer enforces append-only
//! so a compromised process cannot rewrite history without leaving evidence.

use std::error::Error;
use std::fmt;

use crate::{UnixTimestamp, UserId};

/// Longest accepted audit target, bounding what a caller can store.
pub const MAX_TARGET_BYTES: usize = 256;

/// A security-relevant action worth a durable record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    SignIn,
    SignInFailed,
    SignOut,
    Reauthenticate,
    ReauthenticateFailed,
    OsModeChanged,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SignIn => "sign-in",
            Self::SignInFailed => "sign-in-failed",
            Self::SignOut => "sign-out",
            Self::Reauthenticate => "reauthenticate",
            Self::ReauthenticateFailed => "reauthenticate-failed",
            Self::OsModeChanged => "os-mode-changed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuditError> {
        match value {
            "sign-in" => Ok(Self::SignIn),
            "sign-in-failed" => Ok(Self::SignInFailed),
            "sign-out" => Ok(Self::SignOut),
            "reauthenticate" => Ok(Self::Reauthenticate),
            "reauthenticate-failed" => Ok(Self::ReauthenticateFailed),
            "os-mode-changed" => Ok(Self::OsModeChanged),
            _ => Err(AuditError::InvalidAction),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutcome {
    Success,
    Denied,
    Failure,
}

impl AuditOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Denied => "denied",
            Self::Failure => "failure",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuditError> {
        match value {
            "success" => Ok(Self::Success),
            "denied" => Ok(Self::Denied),
            "failure" => Ok(Self::Failure),
            _ => Err(AuditError::InvalidOutcome),
        }
    }
}

/// Who triggered the event. A user is identified by ID, never by a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditActor {
    User(UserId),
    System,
}

impl AuditActor {
    pub fn encode(&self) -> String {
        match self {
            Self::User(user) => format!("user:{user}"),
            Self::System => "system".to_owned(),
        }
    }

    pub fn decode(value: &str) -> Result<Self, AuditError> {
        if value == "system" {
            return Ok(Self::System);
        }
        value
            .strip_prefix("user:")
            .and_then(|raw| UserId::parse(raw).ok())
            .map(Self::User)
            .ok_or(AuditError::InvalidActor)
    }
}

/// An event to append. Built through [`NewAuditEvent::new`] so the target is
/// always validated before it reaches storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuditEvent {
    pub at: UnixTimestamp,
    pub actor: AuditActor,
    pub action: AuditAction,
    pub outcome: AuditOutcome,
    pub target: Option<String>,
}

impl NewAuditEvent {
    pub fn new(
        at: UnixTimestamp,
        actor: AuditActor,
        action: AuditAction,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            at,
            actor,
            action,
            outcome,
            target: None,
        }
    }

    /// Attaches a bounded, non-secret target identifier.
    pub fn with_target(mut self, target: impl Into<String>) -> Result<Self, AuditError> {
        let target = target.into();
        if target.is_empty()
            || target.len() > MAX_TARGET_BYTES
            || target.chars().any(char::is_control)
        {
            return Err(AuditError::InvalidTarget);
        }
        self.target = Some(target);
        Ok(self)
    }
}

/// A stored audit event with its monotonic sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub sequence: u64,
    pub at: UnixTimestamp,
    pub actor: AuditActor,
    pub action: AuditAction,
    pub outcome: AuditOutcome,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditError {
    Unavailable,
    InvalidTarget,
    InvalidActor,
    InvalidAction,
    InvalidOutcome,
}

/// Persistence boundary for the append-only audit log.
pub trait AuditLog: Send + Sync + 'static {
    /// Appends an event and returns its sequence number.
    fn record(&self, event: NewAuditEvent) -> Result<u64, AuditError>;

    /// Most recent events for one user, newest first, bounded by `limit`.
    fn recent(&self, actor: UserId, limit: usize) -> Result<Vec<AuditEvent>, AuditError>;
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("rumahl audit log unavailable"),
            Self::InvalidTarget => f.write_str("rumahl audit target invalid"),
            Self::InvalidActor => f.write_str("rumahl audit actor invalid"),
            Self::InvalidAction => f.write_str("rumahl audit action invalid"),
            Self::InvalidOutcome => f.write_str("rumahl audit outcome invalid"),
        }
    }
}

impl Error for AuditError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip() {
        for action in [
            AuditAction::SignIn,
            AuditAction::SignInFailed,
            AuditAction::SignOut,
            AuditAction::Reauthenticate,
            AuditAction::ReauthenticateFailed,
            AuditAction::OsModeChanged,
        ] {
            assert_eq!(AuditAction::parse(action.as_str()).unwrap(), action);
        }
        assert_eq!(
            AuditAction::parse("unknown").unwrap_err(),
            AuditError::InvalidAction
        );
    }

    #[test]
    fn actor_round_trips_without_secrets() {
        assert_eq!(AuditActor::decode("system").unwrap(), AuditActor::System);
        let user = UserId::new();
        let encoded = AuditActor::User(user).encode();
        assert!(encoded.starts_with("user:"));
        assert_eq!(AuditActor::User(user).encode(), encoded);
        assert_eq!(
            AuditActor::decode(&encoded).unwrap(),
            AuditActor::User(user)
        );
        assert_eq!(
            AuditActor::decode("user:not-a-user").unwrap_err(),
            AuditError::InvalidActor
        );
    }

    #[test]
    fn target_is_bounded_and_rejects_control_characters() {
        let at = UnixTimestamp::from_seconds(1);
        let event = NewAuditEvent::new(
            at,
            AuditActor::System,
            AuditAction::SignIn,
            AuditOutcome::Success,
        );
        assert!(event.clone().with_target("").is_err());
        assert!(event.clone().with_target("a\u{0}b").is_err());
        assert!(event.with_target("x".repeat(MAX_TARGET_BYTES + 1)).is_err());
    }
}
