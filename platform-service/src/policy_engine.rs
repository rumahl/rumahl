//! The local policy engine: catalog keys resolved against the identity store.
//!
//! Every enforcement point asks one question per request; the answers are
//! resolved live from `policy_assignment` + group membership, never cached. A
//! store that cannot be read answers with the catalog default, which is the
//! same value an empty store would give — so a broken store neither grants
//! anything new nor takes working behavior away.
use rumahl_core::{PolicyKey, PolicyValue, UserId, policy_definition};
use rumahl_persistence_sqlite::SqlitePolicyStore;

use crate::ServiceError;
use rumahl_platform_web::{PasswordRules, PolicyEngine};

pub struct LocalPolicyEngine {
    store: SqlitePolicyStore,
}

impl LocalPolicyEngine {
    pub fn open(accounts: &std::path::Path) -> Result<Self, ServiceError> {
        Ok(Self {
            store: SqlitePolicyStore::open(accounts)?,
        })
    }

    /// The effective value of a user-scoped key, or the catalog default.
    fn effective(&self, user: UserId, key: &PolicyKey) -> PolicyValue {
        let default = policy_definition(key)
            .map(|definition| definition.default.to_value())
            .unwrap_or(PolicyValue::Boolean(false));
        self.store.effective(user, key).unwrap_or(default)
    }

    /// The effective value of a device-wide key, or the catalog default.
    fn effective_device(&self, key: &PolicyKey) -> PolicyValue {
        let default = policy_definition(key)
            .map(|definition| definition.default.to_value())
            .unwrap_or(PolicyValue::Boolean(false));
        self.store.effective_device(key).unwrap_or(default)
    }
}

fn key(name: &str) -> PolicyKey {
    PolicyKey::parse(name).expect("catalog key")
}

impl PolicyEngine for LocalPolicyEngine {
    fn advanced_mode_allowed(&self, user: UserId) -> bool {
        self.effective(user, &key("rumahl.os.advanced_mode"))
            .as_bool()
            .unwrap_or(false)
    }

    fn store_install_allowed(&self, user: UserId) -> bool {
        self.effective(user, &key("rumahl.os.allow_store_install"))
            .as_bool()
            .unwrap_or(false)
    }

    fn app_uninstall_allowed(&self, user: UserId) -> bool {
        self.effective(user, &key("rumahl.os.allow_app_uninstall"))
            .as_bool()
            .unwrap_or(false)
    }

    fn home_cross_user_read(&self, actor: UserId) -> bool {
        self.effective(actor, &key("rumahl.home.cross_user_read"))
            .as_bool()
            .unwrap_or(false)
    }

    fn personal_file_quota(&self, user: UserId) -> u64 {
        self.effective(user, &key("rumahl.files.quota_bytes"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_PERSONAL_QUOTA_BYTES)
    }

    fn max_file_bytes(&self, user: UserId) -> u64 {
        self.effective(user, &key("rumahl.files.max_file_bytes"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_MAX_FILE_BYTES)
    }

    fn durable_grants_allowed(&self, user: UserId) -> bool {
        self.effective(user, &key("rumahl.files.durable_grants"))
            .as_bool()
            .unwrap_or(false)
    }

    fn session_lifetime_seconds(&self, user: UserId) -> u64 {
        self.effective(user, &key("rumahl.session.lifetime_seconds"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_SESSION_SECONDS)
    }

    fn password_rules(&self) -> PasswordRules {
        let minimum_characters = self
            .effective_device(&key("rumahl.password.min_length"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_PASSWORD_MIN_LENGTH);
        let required_character_classes = self
            .effective_device(&key("rumahl.password.require_classes"))
            .as_number()
            .unwrap_or(0);
        let attempt_limit = self
            .effective_device(&key("rumahl.password.attempt_limit"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_PASSWORD_ATTEMPT_LIMIT);
        let lockout_seconds = self
            .effective_device(&key("rumahl.password.lockout_seconds"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_PASSWORD_LOCKOUT_SECONDS);
        PasswordRules {
            minimum_characters,
            required_character_classes,
            attempt_limit,
            lockout_seconds,
        }
    }

    fn app_storage_quota(&self) -> u64 {
        self.effective_device(&key("rumahl.apps.storage_quota_bytes"))
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_APP_STORAGE_QUOTA_BYTES)
    }

    fn allowed_publishers(&self) -> Vec<String> {
        self.effective_device(&key("rumahl.apps.allowed_publishers"))
            .as_list()
            .map(|list| list.to_vec())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_persistence_sqlite::SqlitePolicyStore;

    #[test]
    fn defaults_hold_when_nothing_is_assigned() {
        let temp = tempfile::tempdir().unwrap();
        let engine = LocalPolicyEngine::open(&temp.path().join("accounts.sqlite")).unwrap();
        let user = UserId::new();
        assert!(engine.advanced_mode_allowed(user));
        assert!(engine.store_install_allowed(user));
        assert!(engine.app_uninstall_allowed(user));
        assert!(!engine.home_cross_user_read(user));
        assert!(engine.durable_grants_allowed(user));
        assert_eq!(
            engine.personal_file_quota(user),
            rumahl_core::DEFAULT_PERSONAL_QUOTA_BYTES
        );
        assert_eq!(
            engine.max_file_bytes(user),
            rumahl_core::DEFAULT_MAX_FILE_BYTES
        );
        assert_eq!(
            engine.session_lifetime_seconds(user),
            rumahl_core::DEFAULT_SESSION_SECONDS
        );
        assert_eq!(
            engine.password_rules(),
            PasswordRules {
                minimum_characters: rumahl_core::DEFAULT_PASSWORD_MIN_LENGTH,
                required_character_classes: 0,
                attempt_limit: rumahl_core::DEFAULT_PASSWORD_ATTEMPT_LIMIT,
                lockout_seconds: rumahl_core::DEFAULT_PASSWORD_LOCKOUT_SECONDS
            }
        );
        assert_eq!(
            engine.app_storage_quota(),
            rumahl_core::DEFAULT_APP_STORAGE_QUOTA_BYTES
        );
        assert!(engine.allowed_publishers().is_empty());
    }

    #[test]
    fn assignments_change_the_answers() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("accounts.sqlite");
        let user = UserId::new();
        let store = SqlitePolicyStore::open(&path).unwrap();
        store.insert_test_account(user, "alice").unwrap();
        store
            .set_assignment(
                rumahl_core::PolicySubject::User(user),
                key("rumahl.os.advanced_mode"),
                Some(rumahl_core::PolicyValue::Boolean(false)),
            )
            .unwrap();
        store
            .set_assignment(
                rumahl_core::PolicySubject::User(user),
                key("rumahl.session.lifetime_seconds"),
                Some(rumahl_core::PolicyValue::Number(600)),
            )
            .unwrap();
        store
            .set_assignment(
                rumahl_core::PolicySubject::Device,
                key("rumahl.password.min_length"),
                Some(rumahl_core::PolicyValue::Number(24)),
            )
            .unwrap();
        store
            .set_assignment(
                rumahl_core::PolicySubject::Device,
                key("rumahl.password.require_classes"),
                Some(rumahl_core::PolicyValue::Number(3)),
            )
            .unwrap();
        store
            .set_assignment(
                rumahl_core::PolicySubject::Device,
                key("rumahl.apps.allowed_publishers"),
                Some(rumahl_core::PolicyValue::List(vec![
                    "com.rumahl".to_owned(),
                ])),
            )
            .unwrap();
        drop(store);

        let engine = LocalPolicyEngine::open(&path).unwrap();
        assert!(!engine.advanced_mode_allowed(user));
        assert_eq!(engine.session_lifetime_seconds(user), 600);
        assert_eq!(engine.password_rules().minimum_characters, 24);
        assert_eq!(engine.password_rules().required_character_classes, 3);
        assert_eq!(engine.allowed_publishers(), vec!["com.rumahl".to_owned()]);
    }
}
