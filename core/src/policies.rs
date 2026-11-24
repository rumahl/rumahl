//! Administrative policies: device, group and user assignments.
//!
//! A policy is a named, typed setting the platform enforces — the local
//! equivalent of a group policy store. Administrators assign values to the
//! whole device, to a group or to a single user; the effective value for a
//! user is the first assignment found in the order user, group (groups sorted
//! by name), device — otherwise the built-in default. Some policies are
//! device-wide by nature (password rules, the app-storage budget, the
//! publisher allowlist); for those, only the device assignment and the default
//! ever apply. A key that is not in the catalog does not exist: assignments to
//! unknown keys are refused instead of silently ignored.
//!
//! Value types mirror what policy editors need: booleans, numbers with
//! inclusive bounds, single-line text with a length range, and lists of names
//! (for example the trusted-publisher allowlist) with a bounded item count.
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

    /// The `<area>` of `rumahl.<area>.<name>`, used to group the editor.
    pub fn area(&self) -> &str {
        self.0.split('.').nth(1).unwrap_or_default()
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
    Text,
    List,
}

impl PolicyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::Text => "text",
            Self::List => "list",
        }
    }
}

/// Separator for the list storage encoding; never a legal policy value byte.
const LIST_SEPARATOR: char = '\u{1f}';

/// One assigned policy value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    Boolean(bool),
    Number(u64),
    Text(String),
    List(Vec<String>),
}

impl PolicyValue {
    pub fn kind(&self) -> PolicyKind {
        match self {
            Self::Boolean(_) => PolicyKind::Boolean,
            Self::Number(_) => PolicyKind::Number,
            Self::Text(_) => PolicyKind::Text,
            Self::List(_) => PolicyKind::List,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<u64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// Storage encoding: `b:true`, `n:1024`, `t:hint`, `l:a␟b␟c`.
    pub fn encode(&self) -> String {
        match self {
            Self::Boolean(value) => format!("b:{value}"),
            Self::Number(value) => format!("n:{value}"),
            Self::Text(value) => format!("t:{value}"),
            Self::List(values) => {
                let separator = LIST_SEPARATOR.to_string();
                format!("l:{}", values.join(&separator))
            }
        }
    }

    pub fn decode(value: &str) -> Option<Self> {
        if let Some(rest) = value.strip_prefix("b:") {
            return match rest {
                "true" => Some(Self::Boolean(true)),
                "false" => Some(Self::Boolean(false)),
                _ => None,
            };
        }
        if let Some(rest) = value.strip_prefix("n:") {
            return rest.parse::<u64>().ok().map(Self::Number);
        }
        if let Some(rest) = value.strip_prefix("t:") {
            if rest.chars().any(char::is_control) {
                return None;
            }
            return Some(Self::Text(rest.to_owned()));
        }
        let rest = value.strip_prefix("l:")?;
        if rest.is_empty() {
            return Some(Self::List(Vec::new()));
        }
        let values: Option<Vec<String>> = rest
            .split(LIST_SEPARATOR)
            .map(|item| (!item.chars().any(char::is_control)).then(|| item.to_owned()))
            .collect();
        values.map(Self::List)
    }
}

/// A default small enough to keep `Copy` on catalog entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDefault {
    Boolean(bool),
    Number(u64),
    Text(&'static str),
    List(&'static [&'static str]),
}

impl PolicyDefault {
    pub fn kind(self) -> PolicyKind {
        match self {
            Self::Boolean(_) => PolicyKind::Boolean,
            Self::Number(_) => PolicyKind::Number,
            Self::Text(_) => PolicyKind::Text,
            Self::List(_) => PolicyKind::List,
        }
    }

    pub fn to_value(self) -> PolicyValue {
        match self {
            Self::Boolean(value) => PolicyValue::Boolean(value),
            Self::Number(value) => PolicyValue::Number(value),
            Self::Text(value) => PolicyValue::Text(value.to_owned()),
            Self::List(values) => {
                PolicyValue::List(values.iter().map(|value| (*value).to_owned()).collect())
            }
        }
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

/// One catalog entry: the key, its type, its default and its bounds.
///
/// The bounds mean: `Number` — inclusive value range; `Text` — inclusive
/// character-count range; `List` — inclusive item-count range (each item is
/// additionally capped at 255 bytes). A bound of zero means unbounded on that
/// side, except where the type itself forbids it.
#[derive(Debug, Clone, Copy)]
pub struct PolicyDefinition {
    pub key: &'static str,
    pub kind: PolicyKind,
    pub default: PolicyDefault,
    /// Inclusive lower bound; zero means unbounded.
    pub minimum: u64,
    /// Inclusive upper bound; zero means unbounded.
    pub maximum: u64,
    pub description: &'static str,
}

impl PolicyDefinition {
    pub fn key(&self) -> PolicyKey {
        // Catalog keys are compile-time constants and validated by the test
        // below; a malformed entry would be a build error, not a runtime one.
        PolicyKey::parse(self.key).expect("catalog policy key is valid")
    }

    /// The `<area>` of the key, for grouping the editor.
    pub fn area(&self) -> &'static str {
        self.key.split('.').nth(1).unwrap_or_default()
    }
}

/// The default personal-files budget: 128 MiB per account.
pub const DEFAULT_PERSONAL_QUOTA_BYTES: u64 = 128 * 1024 * 1024;
/// The default per-app data budget: 256 MiB per installed app.
pub const DEFAULT_APP_STORAGE_QUOTA_BYTES: u64 = 256 * 1024 * 1024;
/// The default per-file cap for shell uploads and writes: 16 MiB.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// The default browser session lifetime: 12 hours.
pub const DEFAULT_SESSION_SECONDS: u64 = 12 * 60 * 60;
/// The default minimum password length (matches the account defaults).
pub const DEFAULT_PASSWORD_MIN_LENGTH: u64 = 15;
/// The default failed-login attempts before a temporary lockout.
pub const DEFAULT_PASSWORD_ATTEMPT_LIMIT: u64 = 5;
/// The default lockout duration in seconds.
pub const DEFAULT_PASSWORD_LOCKOUT_SECONDS: u64 = 30;

/// Every policy the platform knows, grouped by area. Assignments outside this
/// catalog are refused by the store, the API and the resolution alike.
///
/// Device-wide keys (`password.*`, `apps.storage_quota_bytes`,
/// `apps.allowed_publishers`) ignore group and user assignments: they describe
/// the machine, not a viewer.
pub fn policy_catalog() -> &'static [PolicyDefinition] {
    &[
        // Password and lockout — enforced wherever a credential is set or
        // checked (account creation, admin resets, CLI provisioning, login).
        PolicyDefinition {
            key: "rumahl.password.min_length",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_PASSWORD_MIN_LENGTH),
            minimum: 6,
            maximum: 128,
            description: "Minimum password length for every account.",
        },
        PolicyDefinition {
            key: "rumahl.password.require_classes",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(0),
            minimum: 0,
            maximum: 4,
            description: "How many character classes (lowercase, uppercase, digits, symbols) a password must use.",
        },
        PolicyDefinition {
            key: "rumahl.password.attempt_limit",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_PASSWORD_ATTEMPT_LIMIT),
            minimum: 1,
            maximum: 100,
            description: "Failed login attempts before the account locks out temporarily.",
        },
        PolicyDefinition {
            key: "rumahl.password.lockout_seconds",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_PASSWORD_LOCKOUT_SECONDS),
            minimum: 5,
            maximum: 3600,
            description: "How long a locked-out account stays locked.",
        },
        // Browser sessions.
        PolicyDefinition {
            key: "rumahl.session.lifetime_seconds",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_SESSION_SECONDS),
            minimum: 300,
            maximum: 2_592_000,
            description: "How long a browser session stays valid; users sign in again afterwards.",
        },
        // Exposure and who may change the device's software.
        PolicyDefinition {
            key: "rumahl.os.advanced_mode",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Advanced exposure mode: installed apps and the system tree stay reachable in the file explorer and the mode can be raised.",
        },
        PolicyDefinition {
            key: "rumahl.os.allow_store_install",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Who may install packages. Off: only administrators can import apps.",
        },
        PolicyDefinition {
            key: "rumahl.os.allow_app_uninstall",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Who may remove installed apps. Off: only administrators can uninstall.",
        },
        PolicyDefinition {
            key: "rumahl.os.allow_full_access_themes",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Allow themes that request full shell access. Off: code themes must run sandboxed and full access is unavailable.",
        },
        // Cross-account home access.
        PolicyDefinition {
            key: "rumahl.home.cross_user_read",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(false),
            minimum: 0,
            maximum: 0,
            description: "Administrators may open and read other accounts' home directories, read-only; every access is audited.",
        },
        // Personal files.
        PolicyDefinition {
            key: "rumahl.files.quota_bytes",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_PERSONAL_QUOTA_BYTES),
            minimum: 16 * 1024 * 1024,
            maximum: 64 * 1024 * 1024 * 1024,
            description: "Personal files budget per account, in bytes.",
        },
        PolicyDefinition {
            key: "rumahl.files.max_file_bytes",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_MAX_FILE_BYTES),
            minimum: 1024 * 1024,
            maximum: 256 * 1024 * 1024,
            description: "Largest single file an account may store, in bytes.",
        },
        PolicyDefinition {
            key: "rumahl.files.durable_grants",
            kind: PolicyKind::Boolean,
            default: PolicyDefault::Boolean(true),
            minimum: 0,
            maximum: 0,
            description: "Whether app consent may outlive the session. Off: users can only grant file access for the current session.",
        },
        // Apps (device-wide).
        PolicyDefinition {
            key: "rumahl.apps.storage_quota_bytes",
            kind: PolicyKind::Number,
            default: PolicyDefault::Number(DEFAULT_APP_STORAGE_QUOTA_BYTES),
            minimum: 16 * 1024 * 1024,
            maximum: 16 * 1024 * 1024 * 1024,
            description: "Private data budget per installed app, in bytes (device-wide).",
        },
        PolicyDefinition {
            key: "rumahl.apps.allowed_publishers",
            kind: PolicyKind::List,
            default: PolicyDefault::List(&[]),
            minimum: 0,
            maximum: 64,
            description: "Trusted publishers whose packages may be installed, one id per line (for example com.rumahl). Empty: every publisher in the trust store.",
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
            .map(|assignment| assignment.value.clone())
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
        .map(|definition| definition.default.to_value())
        .unwrap_or(PolicyValue::Boolean(false))
}

/// Resolves a device-wide key: only the device assignment and the catalog
/// default ever apply — the machine's rules must not depend on who is asking.
pub fn resolve_device_policy(key: &PolicyKey, assignments: &[PolicyAssignment]) -> PolicyValue {
    let definition = policy_definition(key);
    if let Some(value) = assignments
        .iter()
        .find(|assignment| assignment.subject == PolicySubject::Device && &assignment.key == key)
        .map(|assignment| assignment.value.clone())
    {
        return value;
    }
    definition
        .map(|definition| definition.default.to_value())
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
    let bound = |value: u64, at_least: u64, at_most: u64| -> bool {
        (at_least == 0 || value >= at_least) && (at_most == 0 || value <= at_most)
    };
    match &value {
        PolicyValue::Number(number) => {
            if !bound(*number, definition.minimum, definition.maximum) {
                return Err(PolicyKeyError::InvalidCharacter);
            }
        }
        PolicyValue::Text(text) => {
            let characters = text.chars().count() as u64;
            if !bound(characters, definition.minimum, definition.maximum)
                || text.chars().any(char::is_control)
                || text.contains(LIST_SEPARATOR)
            {
                return Err(PolicyKeyError::InvalidCharacter);
            }
        }
        PolicyValue::List(values) => {
            if !bound(values.len() as u64, definition.minimum, definition.maximum) {
                return Err(PolicyKeyError::InvalidCharacter);
            }
            for value in values {
                if value.len() > 255
                    || value.chars().any(char::is_control)
                    || value.contains(LIST_SEPARATOR)
                    || value.is_empty()
                {
                    return Err(PolicyKeyError::InvalidCharacter);
                }
            }
        }
        PolicyValue::Boolean(_) => {}
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
        assert_eq!(
            key("rumahl.os.advanced_mode").as_str(),
            "rumahl.os.advanced_mode"
        );
        assert_eq!(key("rumahl.os.advanced_mode").area(), "os");
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
            assert!(!definition.area().is_empty());
        }
        // Every area is grouped; keys are unique.
        let mut areas: Vec<&str> = policy_catalog().iter().map(|d| d.area()).collect();
        areas.sort();
        areas.dedup();
        assert_eq!(
            areas,
            ["apps", "files", "home", "os", "password", "session"]
        );
        let mut keys: Vec<&str> = policy_catalog().iter().map(|d| d.key).collect();
        keys.sort();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count);
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
        assert_eq!(
            PolicyValue::decode("t:hello world"),
            Some(PolicyValue::Text("hello world".to_owned()))
        );
        assert_eq!(PolicyValue::decode("t:a\u{1f}b"), None);
        assert_eq!(
            PolicyValue::decode("l:com.rumahl\u{1f}org.example"),
            Some(PolicyValue::List(vec![
                "com.rumahl".to_owned(),
                "org.example".to_owned()
            ]))
        );
        assert_eq!(
            PolicyValue::decode("l:"),
            Some(PolicyValue::List(Vec::new()))
        );
        for value in [
            PolicyValue::Boolean(true),
            PolicyValue::Number(7),
            PolicyValue::Text("hint".to_owned()),
            PolicyValue::List(vec!["a".to_owned(), "b".to_owned()]),
        ] {
            assert_eq!(PolicyValue::decode(&value.encode()), Some(value.clone()));
        }
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
            resolve_policy(&policy, user, &groups, std::slice::from_ref(&foreign)),
            PolicyValue::Boolean(true)
        );
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
    fn device_policies_ignore_user_and_group_assignments() {
        let user = UserId::new();
        let group = GroupId::new();
        let rules = key("rumahl.password.min_length");
        let assignments = [
            assignment(
                PolicySubject::User(user),
                "rumahl.password.min_length",
                PolicyValue::Number(8),
            ),
            assignment(
                PolicySubject::Group(group),
                "rumahl.password.min_length",
                PolicyValue::Number(6),
            ),
            assignment(
                PolicySubject::Device,
                "rumahl.password.min_length",
                PolicyValue::Number(24),
            ),
        ];
        // The device assignment wins; user and group assignments for a
        // device-wide key are dead letters for the resolution.
        assert_eq!(
            resolve_device_policy(&rules, &assignments),
            PolicyValue::Number(24)
        );
        // Without one, the catalog default applies.
        assert_eq!(
            resolve_device_policy(&rules, &[]),
            PolicyValue::Number(DEFAULT_PASSWORD_MIN_LENGTH)
        );
        assert_eq!(
            resolve_device_policy(&key("rumahl.apps.allowed_publishers"), &[]),
            PolicyValue::List(Vec::new())
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
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.password.min_length"),
                PolicyValue::Number(12)
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
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.password.min_length"),
                PolicyValue::Number(2)
            )
            .is_err()
        );
    }

    #[test]
    fn text_and_list_values_carry_their_own_bounds() {
        // The publisher allowlist accepts bounded, well-formed items.
        assert!(
            validate_assignment(
                PolicySubject::Device,
                key("rumahl.apps.allowed_publishers"),
                PolicyValue::List(vec!["com.rumahl".to_owned(), "org.example".to_owned()])
            )
            .is_ok()
        );
        for invalid in [
            PolicyValue::List(vec![String::new()]),
            PolicyValue::List(vec!["a\u{1f}b".to_owned()]),
            PolicyValue::List(vec!["x".repeat(256)]),
            PolicyValue::List((0..65).map(|i| i.to_string()).collect()),
            PolicyValue::Text("com.rumahl".to_owned()),
        ] {
            assert!(
                validate_assignment(
                    PolicySubject::Device,
                    key("rumahl.apps.allowed_publishers"),
                    invalid
                )
                .is_err()
            );
        }
    }
}
