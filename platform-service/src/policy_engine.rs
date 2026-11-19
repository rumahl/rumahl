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
use rumahl_platform_web::PolicyEngine;

pub struct LocalPolicyEngine {
    store: SqlitePolicyStore,
}

impl LocalPolicyEngine {
    pub fn open(accounts: &std::path::Path) -> Result<Self, ServiceError> {
        Ok(Self {
            store: SqlitePolicyStore::open(accounts)?,
        })
    }

    fn effective(&self, user: UserId, key: &PolicyKey) -> PolicyValue {
        let default = policy_definition(key)
            .map(|definition| definition.default)
            .unwrap_or(PolicyValue::Boolean(false));
        self.store.effective(user, key).unwrap_or(default)
    }
}

impl PolicyEngine for LocalPolicyEngine {
    fn advanced_mode_allowed(&self, user: UserId) -> bool {
        self.effective(user, &advanced_key())
            .as_bool()
            .unwrap_or(false)
    }

    fn home_cross_user_read(&self, actor: UserId) -> bool {
        self.effective(actor, &cross_user_key())
            .as_bool()
            .unwrap_or(false)
    }

    fn personal_file_quota(&self, user: UserId) -> u64 {
        self.effective(user, &quota_key())
            .as_number()
            .unwrap_or(rumahl_core::DEFAULT_PERSONAL_QUOTA_BYTES)
    }
}

fn advanced_key() -> PolicyKey {
    PolicyKey::parse("rumahl.os.advanced_mode").expect("catalog key")
}

fn cross_user_key() -> PolicyKey {
    PolicyKey::parse("rumahl.home.cross_user_read").expect("catalog key")
}

fn quota_key() -> PolicyKey {
    PolicyKey::parse("rumahl.files.quota_bytes").expect("catalog key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_hold_when_nothing_is_assigned() {
        let temp = tempfile::tempdir().unwrap();
        let engine = LocalPolicyEngine::open(&temp.path().join("accounts.sqlite")).unwrap();
        let user = UserId::new();
        assert!(engine.advanced_mode_allowed(user));
        assert!(!engine.home_cross_user_read(user));
        assert_eq!(
            engine.personal_file_quota(user),
            rumahl_core::DEFAULT_PERSONAL_QUOTA_BYTES
        );
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
                advanced_key(),
                Some(rumahl_core::PolicyValue::Boolean(false)),
            )
            .unwrap();
        let engine = LocalPolicyEngine::open(&path).unwrap();
        assert!(!engine.advanced_mode_allowed(user));
        assert!(!engine.home_cross_user_read(user));
    }
}
