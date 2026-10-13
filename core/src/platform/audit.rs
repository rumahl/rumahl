//! Append-only security audit trail.
//!
//! The audit log is the system's central record of *what happened*, never secret
//! material: no passwords, session tokens, credentials, authorization codes or
//! database queries. Targets are bounded, non-secret identifiers. The persistence
//! layer enforces append-only so a compromised process cannot rewrite history
//! without leaving evidence.
//!
//! Every actor in the system reports into this log: users, the system itself,
//! background services and applications/programs. That makes it possible to
//! answer *what* was triggered, *when*, *who* did it and *through what*.

use std::error::Error;
use std::fmt;

use crate::{UnixTimestamp, UserId};

/// Longest accepted audit target, bounding what a caller can store.
pub const MAX_TARGET_BYTES: usize = 256;

/// A security- or health-relevant action worth a durable record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    // Authentication / authorization.
    SignIn,
    SignInFailed,
    SignOut,
    Reauthenticate,
    ReauthenticateFailed,
    OsModeChanged,
    // Application + runtime lifecycle.
    AppLaunched,
    AppLaunchFailed,
    AppRuntimeStarted,
    AppRuntimeStopped,
    AppInstalled,
    AppUninstalled,
    AppImported,
    // Capability use by sandboxed programs.
    CapabilityInvoked,
    CapabilityDenied,
    // Health, crashes and self-healing.
    AppCrashed,
    AppUnresponsive,
    ServiceStarted,
    ServiceStopped,
    ServiceStale,
    ServiceRecovered,
    SelfHeal,
    Maintenance,
    SystemStarted,
    SystemStopped,
    // Arbitrary, caller-supplied activity from a program/service.
    Activity,
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
            Self::AppLaunched => "app-launched",
            Self::AppLaunchFailed => "app-launch-failed",
            Self::AppRuntimeStarted => "app-runtime-started",
            Self::AppRuntimeStopped => "app-runtime-stopped",
            Self::AppInstalled => "app-installed",
            Self::AppUninstalled => "app-uninstalled",
            Self::AppImported => "app-imported",
            Self::CapabilityInvoked => "capability-invoked",
            Self::CapabilityDenied => "capability-denied",
            Self::AppCrashed => "app-crashed",
            Self::AppUnresponsive => "app-unresponsive",
            Self::ServiceStarted => "service-started",
            Self::ServiceStopped => "service-stopped",
            Self::ServiceStale => "service-stale",
            Self::ServiceRecovered => "service-recovered",
            Self::SelfHeal => "self-heal",
            Self::Maintenance => "maintenance",
            Self::SystemStarted => "system-started",
            Self::SystemStopped => "system-stopped",
            Self::Activity => "activity",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AuditError> {
        let action = match value {
            "sign-in" => Self::SignIn,
            "sign-in-failed" => Self::SignInFailed,
            "sign-out" => Self::SignOut,
            "reauthenticate" => Self::Reauthenticate,
            "reauthenticate-failed" => Self::ReauthenticateFailed,
            "os-mode-changed" => Self::OsModeChanged,
            "app-launched" => Self::AppLaunched,
            "app-launch-failed" => Self::AppLaunchFailed,
            "app-runtime-started" => Self::AppRuntimeStarted,
            "app-runtime-stopped" => Self::AppRuntimeStopped,
            "app-installed" => Self::AppInstalled,
            "app-uninstalled" => Self::AppUninstalled,
            "app-imported" => Self::AppImported,
            "capability-invoked" => Self::CapabilityInvoked,
            "capability-denied" => Self::CapabilityDenied,
            "app-crashed" => Self::AppCrashed,
            "app-unresponsive" => Self::AppUnresponsive,
            "service-started" => Self::ServiceStarted,
            "service-stopped" => Self::ServiceStopped,
            "service-stale" => Self::ServiceStale,
            "service-recovered" => Self::ServiceRecovered,
            "self-heal" => Self::SelfHeal,
            "maintenance" => Self::Maintenance,
            "system-started" => Self::SystemStarted,
            "system-stopped" => Self::SystemStopped,
            "activity" => Self::Activity,
            _ => return Err(AuditError::InvalidAction),
        };
        Ok(action)
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

/// Who triggered the event. Users are identified by ID, never by a secret;
/// services and programs by a bounded, non-secret identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditActor {
    User(UserId),
    System,
    Service(String),
    App(String),
    /// The operating system layer itself (boot, services, supervisor, updates).
    /// Always surfaced to users as "rumahl OS", never by an internal name.
    Os,
}

fn valid_actor_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_TARGET_BYTES && !name.chars().any(char::is_control)
}

impl AuditActor {
    pub fn encode(&self) -> String {
        match self {
            Self::User(user) => format!("user:{user}"),
            Self::System => "system".to_owned(),
            Self::Service(name) => format!("service:{name}"),
            Self::App(id) => format!("app:{id}"),
            Self::Os => "os".to_owned(),
        }
    }

    pub fn decode(value: &str) -> Result<Self, AuditError> {
        if value == "system" {
            return Ok(Self::System);
        }
        if value == "os" {
            return Ok(Self::Os);
        }
        if let Some(raw) = value.strip_prefix("user:") {
            return UserId::parse(raw)
                .map(Self::User)
                .map_err(|_| AuditError::InvalidActor);
        }
        if let Some(raw) = value.strip_prefix("service:") {
            return valid_actor_name(raw)
                .then(|| Self::Service(raw.to_owned()))
                .ok_or(AuditError::InvalidActor);
        }
        if let Some(raw) = value.strip_prefix("app:") {
            return valid_actor_name(raw)
                .then(|| Self::App(raw.to_owned()))
                .ok_or(AuditError::InvalidActor);
        }
        Err(AuditError::InvalidActor)
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

/// A filter for reading the system-wide audit log (the event monitor).
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub action: Option<AuditAction>,
    pub actor: Option<AuditActor>,
    pub outcome: Option<AuditOutcome>,
    pub target: Option<String>,
    /// Only events at or after this time.
    pub since: Option<UnixTimestamp>,
    /// Bounded by the adapter; newest first.
    pub limit: usize,
    pub offset: usize,
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

    /// Most recent events across the whole system, newest first. This is what the
    /// recovery / maintenance hub reads.
    fn recent_system(&self, limit: usize) -> Result<Vec<AuditEvent>, AuditError>;

    /// Most recent events whose target matches exactly, newest first.
    fn recent_target(&self, target: &str, limit: usize) -> Result<Vec<AuditEvent>, AuditError>;

    /// System-wide events matching a filter, newest first. Backs the event monitor.
    fn query_system(&self, query: &AuditQuery) -> Result<Vec<AuditEvent>, AuditError>;
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
            AuditAction::AppLaunched,
            AuditAction::AppLaunchFailed,
            AuditAction::AppRuntimeStarted,
            AuditAction::AppRuntimeStopped,
            AuditAction::AppInstalled,
            AuditAction::AppUninstalled,
            AuditAction::AppImported,
            AuditAction::CapabilityInvoked,
            AuditAction::CapabilityDenied,
            AuditAction::AppCrashed,
            AuditAction::AppUnresponsive,
            AuditAction::ServiceStarted,
            AuditAction::ServiceStopped,
            AuditAction::ServiceStale,
            AuditAction::ServiceRecovered,
            AuditAction::SelfHeal,
            AuditAction::Maintenance,
            AuditAction::SystemStarted,
            AuditAction::SystemStopped,
            AuditAction::Activity,
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
        assert_eq!(
            AuditActor::decode("service:renderer").unwrap(),
            AuditActor::Service("renderer".to_owned())
        );
        assert_eq!(
            AuditActor::decode("app:com.example.notes").unwrap(),
            AuditActor::App("com.example.notes".to_owned())
        );
        assert_eq!(AuditActor::decode("os").unwrap(), AuditActor::Os);
        assert_eq!(
            AuditActor::decode("service:").unwrap_err(),
            AuditError::InvalidActor
        );
        assert_eq!(
            AuditActor::decode("nonsense").unwrap_err(),
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
