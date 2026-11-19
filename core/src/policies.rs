//! Administrative policies: device, group and user assignments.
//!
//! A policy is a named, typed setting the platform enforces. Administrators
//! assign values to the whole device, to a group or to a single user; the
//! effective value for a user is the first assignment found in the order
//! user, group (groups sorted by name), device — otherwise the built-in
//! default. A key that is not in the catalog does not exist: assignments to
//! unknown keys are refused instead of silently ignored.
//!
//! This is the local equivalent of a group policy store: small, typed and
//! resolved on every enforcement point, never cached beyond one request.
use crate::identity::{GroupId, UserId};

/// One policy key, `rumahl.<area>.<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolicyKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKeyError {
    Empty,
    TooLong,
    InvalidCharacter,
    NotARumahlPolicy,
}

impl PolicyKey {
    pub const MAX_LENGTH: usize = 64;

    pub fn parse(value: &str) -> Result<Self, PolicyKeyError> {
        if value.is_empty() {
            return Err(PolicyKeyError::Empty);
        }
        if value.len() > Self::MAX_LENGTH {
            return Err(PolicyKeyError::TooLong);
        }
        let mut parts = value.split('.');
        let (area, name) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some("rumahl"), Some(area), Some(name), None)
                if !area.is_empty() && !name.is_empty() =>
            {
                (area, name)
            }
            _ => return Err(PolicyKeyError::NotARumahlPolicy),
        };
        for part in [area, name] {
            if !part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(PolicyKeyError::InvalidCharacter);
            }
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PolicyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for PolicyKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "policy key is empty"),
            Self::TooLong => write!(f, "policy key is too long"),
            Self::InvalidCharacter => write!(f, "policy key has an invalid character"),
            Self::NotARumahlPolicy => write!(f, "policy key must look like rumahl.<area>.<name>"),
        }
    }
}

impl std::error::Error for PolicyKeyError {}

/// The value type a policy key accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    Boolean,
    Number,
}

impl PolicyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Number => "number",
        }
    }
}

/// One assigned policy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyValue {
    Boolean(bool),
    Number(u64),
}

impl PolicyValue {
    pub fn kind(self) -> PolicyKind {
        match self {
            Self::Boolean(_) => PolicyKind::Boolean,
            Self::Number(_) => PolicyKind::Number,
        }
    }

    pub fn as_bool(self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(value),
            Self::Number(_) => None,
        }
    }

    pub fn as_number(self) -> Option<u64> {
        match self {
            Self::Number(value) => Some(value),
            Self::Boolean(_) => None,
        }
    }

    /// Storage encoding: `b:true`, `n:1024`.
    pub fn encode(self) -> String {
        match self {
            Self::Boolean(value) => format!("b:{value}"),
            Self::Number(value) => format!("n:{value}"),
        }
    }

    pub fn decode(value: &str) -> Option<Self> {
        match value.strip_prefix("b:") {
            Some("true") => return Some(Self::Boolean(true)),
            Some("false") => return Some(Self::Boolean(false)),
            _ => {}
        }
        value
            .strip_prefix("n:")
            .and_then(|number| number.parse::<u64>().ok())
            .map(Self::Number)
    }
}

/// Who a policy assignment applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySubject {
    Device,
    Group(GroupId),
    User(UserId),
}

impl PolicySubject {
    /// Storage/API encoding: `device`, `group:<uuid>`, `user:<uuid>`.
    pub fn encode(&self) -> String {
        match self {
            Self::Device => "device".to_owned(),
            Self::Group(id) => format!("group:{id}"),
            Self::User(id) => format!("user:{id}"),
        }
    }

    pub fn decode(value: &str) -> Option<Self> {
        if value == "device" {
            return Some(Self::Device);
        }
        let (kind, id) = value.split_once(':')?;
        match kind {
            "group" => GroupId::parse(id).ok().map(Self::Group),
            "user" => UserId::parse(id).ok().map(Self::User),
            _ => None,
        }
    }
}

/// One catalog entry: the key, its type, its default and what it governs.
#[derive(Debug, Clone, Copy)]
pub struct PolicyDefinition {
    pub key: &'static str,
    pub kind: PolicyKind,
    pub default: PolicyValue,
    /// Inclusive lower bound for number policies; zero means unbounded.
    pub minimum: u64,
    /// Inclusive upper bound for number policies; zero means unbounded.
    pub maximum: u64,
    pub description: &'static str,
}

impl PolicyDefinition {
    pub fn key(&self) -> PolicyKey {
        // Catalog keys are compile-time constants and validated by the test
        // below; a malformed entry would be a build error, not a runtime one.
        PolicyKey::parse(self.key).expect("catalog policy key is valid")
    }
}

/// The default personal-files budget: 128 MiB per account.
pub const DEFAULT_PERSONAL_QUOTA_BYTES: u64 = 128 * 1024 * 1024;

/// Every policy the platform knows. Assignments outside this catalog are
/// refused by the store, the API and the resolution alike.
pub fn policy_catalog() -> &'static [PolicyDefinition] {
    &[
        PolicyDefinition {
            key: "rumahl.os.advanced_mode",
            kind: PolicyKind::Boolean,
            default: PolicyValue::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Advanced exposure mode: installed apps and the system tree stay reachable in the file explorer and the mode can be raised.",
        },
        PolicyDefinition {
            key: "rumahl.home.cross_user_read",
            kind: PolicyKind::Boolean,
            default: PolicyValue::Boolean(false),
            minimum: 0,
            maximum: 0,
            description: "Administrators may open and read other accounts' home directories, read-only; every access is audited.",
        },
        PolicyDefinition {
            key: "rumahl.files.quota_bytes",
            kind: PolicyKind::Number,
            default: PolicyValue::Number(DEFAULT_PERSONAL_QUOTA_BYTES),
            minimum: 16 * 1024 * 1024,
            maximum: 64 * 1024 * 1024 * 1024,
            description: "Personal files budget per account, in bytes.",
        },
    ]
}

/// Looks a key up in the catalog.
pub fn policy_definition(key: &PolicyKey) -> Option<&'static PolicyDefinition> {
    policy_catalog()
        .iter()
        .find(|definition| definition.key == key.as_str())
}

/// One stored assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAssignment {
    pub subject: PolicySubject,
    pub key: PolicyKey,
    pub value: PolicyValue,
}

/// Resolves the effective value of a key for one user.
///
/// Precedence: the user's own assignment, then the user's groups in name
/// order (first match wins, so overlapping groups stay deterministic), then
/// the device assignment, then the catalog default.
pub fn resolve_policy(
    key: &PolicyKey,
    user: UserId,
    groups: &[(GroupId, String)],
    assignments: &[PolicyAssignment],
) -> PolicyValue {
    let definition = policy_definition(key);
    let find = |subject: &PolicySubject| {
        assignments
            .iter()
            .find(|assignment| &assignment.subject == subject && &assignment.key == key)
            .map(|assignment| assignment.value)
    };
    if let Some(value) = find(&PolicySubject::User(user)) {
        return value;
    }
    let mut ordered: Vec<&(GroupId, String)> = groups.iter().collect();
    ordered.sort_by(|left, right| left.1.cmp(&right.1));
    for (id, _) in ordered {
        if let Some(value) = find(&PolicySubject::Group(*id)) {
            return value;
        }
    }
    if let Some(value) = find(&PolicySubject::Device) {
        return value;
    }
    definition
        .map(|definition| definition.default)
        .unwrap_or(PolicyValue::Boolean(false))
}

/// Validates an assignment against the catalog before it is stored: the key
/// must exist, the value must have the right type and stay inside the bounds.
pub fn validate_assignment(
    subject: PolicySubject,
    key: PolicyKey,
    value: PolicyValue,
) -> Result<PolicyAssignment, PolicyKeyError> {
    let Some(definition) = policy_definition(&key) else {
        return Err(PolicyKeyError::NotARumahlPolicy);
    };
    if definition.kind != value.kind() {
        return Err(PolicyKeyError::InvalidCharacter);
    }
    if let PolicyValue::Number(number) = value {
        if definition.minimum > 0 && number < definition.minimum {
            return Err(PolicyKeyError::InvalidCharacter);
        }
        if definition.maximum > 0 && number > definition.maximum {
            return Err(PolicyKeyError::InvalidCharacter);
        }
    }
    Ok(PolicyAssignment {
        subject,
        key,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: &str) -> PolicyKey {
        PolicyKey::parse(value).unwrap()
    }

    fn groups(names: &[&str]) -> Vec<(GroupId, String)> {
        names
            .iter()
            .map(|name| (GroupId::new(), (*name).to_owned()))
            .collect()
    }

    fn assignment(subject: PolicySubject, name: &str, value: PolicyValue) -> PolicyAssignment {
        validate_assignment(subject, key(name), value).unwrap()
    }

    #[test]
    fn keys_must_be_catalogued_rumahl_keys() {
        assert!(key("rumahl.os.advanced_mode").as_str() == "rumahl.os.advanced_mode");
        for invalid in [
            "",
            "os.advanced",
            "rumahl.os",
            "rumahl.",
            "RUMAHL.os.mode",
            "rumahl.os.mode!",
            "a.b.c.d.e",
        ] {
            assert!(PolicyKey::parse(invalid).is_err(), "{invalid}");
        }
        for definition in policy_catalog() {
            assert!(PolicyKey::parse(definition.key).is_ok());
        }
    }

    #[test]
    fn values_round_trip_through_the_storage_encoding() {
        assert_eq!(
            PolicyValue::decode("b:true"),
            Some(PolicyValue::Boolean(true))
        );
        assert_eq!(
            PolicyValue::decode("b:false"),
            Some(PolicyValue::Boolean(false))
        );
        assert_eq!(
            PolicyValue::decode("n:1024"),
            Some(PolicyValue::Number(1024))
        );
        assert_eq!(PolicyValue::decode("n:-1"), None);
        assert_eq!(PolicyValue::decode("true"), None);
    }

    #[test]
    fn subjects_round_trip_and_reject_strangers() {
        let group = GroupId::new();
        let user = UserId::new();
        for subject in [
            PolicySubject::Device,
            PolicySubject::Group(group),
            PolicySubject::User(user),
        ] {
            assert_eq!(PolicySubject::decode(&subject.encode()), Some(subject));
        }
        assert_eq!(PolicySubject::decode("device"), Some(PolicySubject::Device));
        assert_eq!(PolicySubject::decode("group:not-a-uuid"), None);
        assert_eq!(PolicySubject::decode("other:1"), None);
    }

    #[test]
    fn resolution_follows_user_group_device_default() {
        let user = UserId::new();
        let groups = groups(&["staff", "engineering"]);
        let group_id = groups[0].0;
        let other_group = groups[1].0;
        let policy = key("rumahl.os.advanced_mode");

        // Nothing assigned: the catalog default.
        assert_eq!(
            resolve_policy(&policy, user, &groups, &[]),
            PolicyValue::Boolean(true)
        );

        // Device sets the floor; a group overrides it; the user overrides both.
        let device = assignment(
            PolicySubject::Device,
            "rumahl.os.advanced_mode",
            PolicyValue::Boolean(false),
        );
        assert_eq!(
            resolve_policy(&policy, user, &groups, std::slice::from_ref(&device)),
            PolicyValue::Boolean(false)
        );
        let group_assignment = assignment(
            PolicySubject::Group(group_id),
            "rumahl.os.advanced_mode",
            PolicyValue::Boolean(true),
        );
        assert_eq!(
            resolve_policy(
                &policy,
                user,
                &groups,
                &[device.clone(), group_assignment.clone()]
            ),
            PolicyValue::Boolean(true)
        );
        let user_assignment = assignment(
            PolicySubject::User(user),
            "rumahl.os.advanced_mode",
            PolicyValue::Boolean(false),
        );
        assert_eq!(
            resolve_policy(
                &policy,
                user,
                &groups,
                &[device, group_assignment, user_assignment]
            ),
            PolicyValue::Boolean(false)
        );

        // A group the user is not in never applies.
        let foreign = assignment(
            PolicySubject::Group(GroupId::new()),
            "rumahl.os.advanced_mode",
            PolicyValue::Boolean(false),
        );
        assert_eq!(
            resolve_policy(&policy, user, &groups, &[foreign]),
            PolicyValue::Boolean(true)
        );
        let _ = other_group;
    }

    #[test]
    fn overlapping_groups_resolve_by_group_name() {
        let user = UserId::new();
        let groups = groups(&["staff", "admins"]);
        // "admins" sorts before "staff", so its assignment wins.
        let admin_group = groups[1].0;
        let staff_group = groups[0].0;
        let admin = assignment(
            PolicySubject::Group(admin_group),
            "rumahl.files.quota_bytes",
            PolicyValue::Number(64 * 1024 * 1024),
        );
        let staff = assignment(
            PolicySubject::Group(staff_group),
            "rumahl.files.quota_bytes",
            PolicyValue::Number(32 * 1024 * 1024),
        );
        assert_eq!(
            resolve_policy(
                &key("rumahl.files.quota_bytes"),
                user,
                &groups,
                &[admin, staff]
            ),
            PolicyValue::Number(64 * 1024 * 1024)
        );
    }

    #[test]
    fn assignments_are_validated_against_the_catalog() {
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.os.advanced_mode"),
                PolicyValue::Boolean(false)
            )
            .is_ok()
        );
        // Wrong type, unknown key and out-of-range numbers are refused.
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.os.advanced_mode"),
                PolicyValue::Number(1)
            )
            .is_err()
        );
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.os.unknown"),
                PolicyValue::Boolean(true)
            )
            .is_err()
        );
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.files.quota_bytes"),
                PolicyValue::Number(1)
            )
            .is_err()
        );
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.files.quota_bytes"),
                PolicyValue::Number(u64::MAX)
            )
            .is_err()
        );
    }
}
