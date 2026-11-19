//! Account groups and their policy assignments, stored in the identity database.
//!
//! Groups are how a policy reaches many accounts at once: an assignment for a
//! group applies to every member unless the user has their own assignment.
//! Names are unique case-insensitively, membership is a plain many-to-many
//! table with cascading deletes, and policy assignments are validated against
//! the core catalog before anything is stored.
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{
    GroupId, PolicyAssignment, PolicyKey, PolicyKeyError, PolicySubject, PolicyValue, UserId,
    resolve_policy,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::identity_schema;

#[derive(Debug)]
pub enum PolicyStoreError {
    Storage,
    LockPoisoned,
    /// The assignment does not match the policy catalog.
    InvalidPolicy(PolicyKeyError),
    /// The subject does not exist: unknown user or group.
    UnknownSubject,
    UnknownGroup,
    DuplicateGroupName,
}

impl std::fmt::Display for PolicyStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage => write!(f, "policy storage failed"),
            Self::LockPoisoned => write!(f, "policy storage is busy"),
            Self::InvalidPolicy(error) => write!(f, "policy assignment is invalid: {error}"),
            Self::UnknownSubject => write!(f, "policy subject does not exist"),
            Self::UnknownGroup => write!(f, "group does not exist"),
            Self::DuplicateGroupName => write!(f, "group name is already taken"),
        }
    }
}

impl std::error::Error for PolicyStoreError {}

/// One group with its members, as the administration API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRecord {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<UserId>,
}

/// Groups and policy assignments over one identity database.
pub struct SqlitePolicyStore {
    connection: Mutex<Connection>,
}

impl SqlitePolicyStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PolicyStoreError> {
        Self::from_connection(Connection::open(path).map_err(|_| PolicyStoreError::Storage)?)
    }

    pub fn open_in_memory() -> Result<Self, PolicyStoreError> {
        Self::from_connection(Connection::open_in_memory().map_err(|_| PolicyStoreError::Storage)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, PolicyStoreError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| PolicyStoreError::Storage)?;
        identity_schema::initialize(&connection).map_err(|_| PolicyStoreError::Storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    // ---- Groups -----------------------------------------------------------

    pub fn create_group(&self, name: &str) -> Result<GroupRecord, PolicyStoreError> {
        if name.is_empty() || name.len() > 64 || name.contains(['/', '\\', '\0']) {
            return Err(PolicyStoreError::InvalidPolicy(
                PolicyKeyError::InvalidCharacter,
            ));
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PolicyStoreError::Storage)?;
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM account_group WHERE name = ?1 COLLATE NOCASE)",
                [name],
                |row| row.get(0),
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        if exists {
            return Err(PolicyStoreError::DuplicateGroupName);
        }
        let id = GroupId::new();
        transaction
            .execute(
                "INSERT INTO account_group (group_id, name) VALUES (?1, ?2)",
                rusqlite::params![id.to_string(), name],
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        transaction
            .commit()
            .map_err(|_| PolicyStoreError::Storage)?;
        Ok(GroupRecord {
            id,
            name: name.to_owned(),
            members: Vec::new(),
        })
    }

    pub fn delete_group(&self, id: &GroupId) -> Result<(), PolicyStoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PolicyStoreError::Storage)?;
        // Group assignments fall away with the group; user and device
        // assignments are untouched.
        transaction
            .execute(
                "DELETE FROM policy_assignment WHERE subject = ?1",
                [format!("group:{id}")],
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        let removed = transaction
            .execute(
                "DELETE FROM account_group WHERE group_id = ?1",
                [id.to_string()],
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        transaction
            .commit()
            .map_err(|_| PolicyStoreError::Storage)?;
        if removed == 0 {
            return Err(PolicyStoreError::UnknownGroup);
        }
        Ok(())
    }

    pub fn groups(&self) -> Result<Vec<GroupRecord>, PolicyStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let mut statement = connection
            .prepare("SELECT group_id, name FROM account_group ORDER BY name")
            .map_err(|_| PolicyStoreError::Storage)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| PolicyStoreError::Storage)?;
        let mut records = Vec::new();
        for row in rows {
            let (group_id, name) = row.map_err(|_| PolicyStoreError::Storage)?;
            let id = GroupId::parse(&group_id).map_err(|_| PolicyStoreError::Storage)?;
            records.push(GroupRecord {
                id,
                name,
                members: Vec::new(),
            });
        }
        drop(statement);
        for record in &mut records {
            record.members = Self::members_of(&connection, &record.id)?;
        }
        Ok(records)
    }

    fn members_of(connection: &Connection, id: &GroupId) -> Result<Vec<UserId>, PolicyStoreError> {
        let mut statement = connection
            .prepare(
                "SELECT user_id FROM account_group_member WHERE group_id = ?1 ORDER BY user_id",
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        let rows = statement
            .query_map([id.to_string()], |row| row.get::<_, String>(0))
            .map_err(|_| PolicyStoreError::Storage)?;
        let mut members = Vec::new();
        for row in rows {
            let user_id = row.map_err(|_| PolicyStoreError::Storage)?;
            members.push(UserId::parse(&user_id).map_err(|_| PolicyStoreError::Storage)?);
        }
        Ok(members)
    }

    /// Adds and removes members in one atomic step, returning the group.
    pub fn update_members(
        &self,
        id: &GroupId,
        add: &[UserId],
        remove: &[UserId],
    ) -> Result<GroupRecord, PolicyStoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PolicyStoreError::Storage)?;
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM account_group WHERE group_id = ?1)",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        if !exists {
            return Err(PolicyStoreError::UnknownGroup);
        }
        for user_id in add {
            let account: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM local_account WHERE user_id = ?1)",
                    [user_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| PolicyStoreError::Storage)?;
            if !account {
                return Err(PolicyStoreError::UnknownSubject);
            }
            transaction
                .execute(
                    "INSERT OR IGNORE INTO account_group_member (group_id, user_id) VALUES (?1, ?2)",
                    rusqlite::params![id.to_string(), user_id.to_string()],
                )
                .map_err(|_| PolicyStoreError::Storage)?;
        }
        for user_id in remove {
            transaction
                .execute(
                    "DELETE FROM account_group_member WHERE group_id = ?1 AND user_id = ?2",
                    rusqlite::params![id.to_string(), user_id.to_string()],
                )
                .map_err(|_| PolicyStoreError::Storage)?;
        }
        let name: String = transaction
            .query_row(
                "SELECT name FROM account_group WHERE group_id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        transaction
            .commit()
            .map_err(|_| PolicyStoreError::Storage)?;
        let members = Self::members_of(&connection, id)?;
        Ok(GroupRecord {
            id: *id,
            name,
            members,
        })
    }

    /// The user's groups with names, in the order the resolver expects.
    pub fn groups_of(&self, user: UserId) -> Result<Vec<(GroupId, String)>, PolicyStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let mut statement = connection
            .prepare(
                "SELECT g.group_id, g.name FROM account_group g
                 JOIN account_group_member m ON m.group_id = g.group_id
                 WHERE m.user_id = ?1
                 ORDER BY g.name",
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        let rows = statement
            .query_map([user.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| PolicyStoreError::Storage)?;
        let mut groups = Vec::new();
        for row in rows {
            let (group_id, name) = row.map_err(|_| PolicyStoreError::Storage)?;
            groups.push((
                GroupId::parse(&group_id).map_err(|_| PolicyStoreError::Storage)?,
                name,
            ));
        }
        Ok(groups)
    }

    // ---- Assignments ------------------------------------------------------

    pub fn assignments(&self) -> Result<Vec<PolicyAssignment>, PolicyStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let mut statement = connection
            .prepare("SELECT subject, key, value FROM policy_assignment ORDER BY subject, key")
            .map_err(|_| PolicyStoreError::Storage)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|_| PolicyStoreError::Storage)?;
        let mut assignments = Vec::new();
        for row in rows {
            let (subject, key, value) = row.map_err(|_| PolicyStoreError::Storage)?;
            let subject = PolicySubject::decode(&subject).ok_or(
                PolicyStoreError::InvalidPolicy(PolicyKeyError::NotARumahlPolicy),
            )?;
            let key = PolicyKey::parse(&key).map_err(PolicyStoreError::InvalidPolicy)?;
            let value = PolicyValue::decode(&value).ok_or(PolicyStoreError::InvalidPolicy(
                PolicyKeyError::InvalidCharacter,
            ))?;
            assignments.push(PolicyAssignment {
                subject,
                key,
                value,
            });
        }
        Ok(assignments)
    }

    /// Stores or clears one assignment after validating it against the catalog.
    pub fn set_assignment(
        &self,
        subject: PolicySubject,
        key: PolicyKey,
        value: Option<PolicyValue>,
    ) -> Result<(), PolicyStoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PolicyStoreError::Storage)?;
        // The subject must exist: assignments for unknown users or groups
        // would resolve for nobody and silently rot.
        let subject_ok = match &subject {
            PolicySubject::Device => true,
            PolicySubject::User(user) => transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM local_account WHERE user_id = ?1)",
                    [user.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| PolicyStoreError::Storage)?,
            PolicySubject::Group(group) => transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM account_group WHERE group_id = ?1)",
                    [group.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| PolicyStoreError::Storage)?,
        };
        if !subject_ok {
            return Err(PolicyStoreError::UnknownSubject);
        }
        let encoded_subject = subject.encode();
        match value {
            Some(value) => {
                rumahl_core::validate_assignment(subject, key.clone(), value)
                    .map_err(PolicyStoreError::InvalidPolicy)?;
                transaction
                    .execute(
                        "INSERT INTO policy_assignment (subject, key, value) VALUES (?1, ?2, ?3)
                         ON CONFLICT(subject, key) DO UPDATE SET value = excluded.value",
                        rusqlite::params![encoded_subject, key.as_str(), value.encode()],
                    )
                    .map_err(|_| PolicyStoreError::Storage)?;
            }
            None => {
                transaction
                    .execute(
                        "DELETE FROM policy_assignment WHERE subject = ?1 AND key = ?2",
                        rusqlite::params![encoded_subject, key.as_str()],
                    )
                    .map_err(|_| PolicyStoreError::Storage)?;
            }
        }
        transaction.commit().map_err(|_| PolicyStoreError::Storage)
    }

    /// The effective value of a key for one user.
    pub fn effective(
        &self,
        user: UserId,
        key: &PolicyKey,
    ) -> Result<PolicyValue, PolicyStoreError> {
        let groups = self.groups_of(user)?;
        let assignments = self.assignments()?;
        Ok(resolve_policy(key, user, &groups, &assignments))
    }

    /// Inserts an account row so user subjects resolve in tests. Production
    /// uses the account administration service, never this helper.
    pub fn insert_test_account(
        &self,
        user: UserId,
        username: &str,
    ) -> Result<(), PolicyStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        connection
            .execute(
                "INSERT INTO local_account (user_id, username, display_name, status)
                 VALUES (?1, ?2, ?3, 'active')",
                rusqlite::params![user.to_string(), username, username],
            )
            .map_err(|_| PolicyStoreError::Storage)?;
        Ok(())
    }

    /// Whether the database still references a group (used by delete paths).
    pub fn group_exists(&self, id: &GroupId) -> Result<bool, PolicyStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| PolicyStoreError::LockPoisoned)?;
        let exists: Option<String> = connection
            .query_row(
                "SELECT group_id FROM account_group WHERE group_id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| PolicyStoreError::Storage)?;
        Ok(exists.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumahl_core::{policy_catalog, policy_definition};

    fn store() -> SqlitePolicyStore {
        let store = SqlitePolicyStore::open_in_memory().unwrap();
        // One account so user subjects exist.
        let connection = store.connection.lock().unwrap();
        connection
            .execute(
                "INSERT INTO local_account (user_id, username, display_name, status)
                 VALUES ('01990000-0000-7000-8000-000000000001', 'alice', 'Alice', 'active')",
                [],
            )
            .unwrap();
        drop(connection);
        store
    }

    fn user() -> UserId {
        UserId::parse("01990000-0000-7000-8000-000000000001").unwrap()
    }

    fn key(name: &str) -> PolicyKey {
        policy_definition(&PolicyKey::parse(name).unwrap())
            .unwrap()
            .key()
    }

    #[test]
    fn groups_manage_membership_and_disappear_cleanly() {
        let store = store();
        let group = store.create_group("Engineering").unwrap();
        assert!(store.create_group("engineering").is_err());
        let empty = store.create_group("Zeta").unwrap();

        let updated = store.update_members(&group.id, &[user()], &[]).unwrap();
        assert_eq!(updated.members, vec![user()]);
        assert_eq!(store.groups_of(user()).unwrap().len(), 1);
        assert_eq!(store.groups().unwrap().len(), 2);
        // Sorted by name: Engineering first.
        assert_eq!(store.groups().unwrap()[0].name, "Engineering");

        // Unknown users and groups refuse.
        assert!(
            store
                .update_members(&group.id, &[UserId::new()], &[])
                .is_err()
        );
        assert!(store.update_members(&GroupId::new(), &[], &[]).is_err());

        store.update_members(&group.id, &[], &[user()]).unwrap();
        assert!(store.groups_of(user()).unwrap().is_empty());
        store.delete_group(&empty.id).unwrap();
        assert!(store.delete_group(&empty.id).is_err());
    }

    #[test]
    fn assignments_are_catalog_validated_and_subject_checked() {
        let store = store();
        let mode = key("rumahl.os.advanced_mode");
        let quota = key("rumahl.files.quota_bytes");

        store
            .set_assignment(
                PolicySubject::Device,
                mode.clone(),
                Some(PolicyValue::Boolean(false)),
            )
            .unwrap();
        // Wrong type and unknown keys never reach storage.
        assert!(
            store
                .set_assignment(
                    PolicySubject::Device,
                    mode.clone(),
                    Some(PolicyValue::Number(1))
                )
                .is_err()
        );
        assert!(
            store
                .set_assignment(
                    PolicySubject::Device,
                    PolicyKey::parse("rumahl.os.unknown").unwrap(),
                    Some(PolicyValue::Boolean(true))
                )
                .is_err()
        );
        // An unknown user subject is refused.
        assert!(
            store
                .set_assignment(
                    PolicySubject::User(UserId::new()),
                    mode.clone(),
                    Some(PolicyValue::Boolean(false))
                )
                .is_err()
        );
        // The number bounds from the catalog apply.
        assert!(
            store
                .set_assignment(
                    PolicySubject::Device,
                    quota.clone(),
                    Some(PolicyValue::Number(1))
                )
                .is_err()
        );

        let assignments = store.assignments().unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].subject, PolicySubject::Device);
        assert_eq!(assignments[0].value, PolicyValue::Boolean(false));

        store
            .set_assignment(PolicySubject::Device, mode.clone(), None)
            .unwrap();
        assert!(store.assignments().unwrap().is_empty());
    }

    #[test]
    fn effective_resolution_follows_user_group_device() {
        let store = store();
        let mode = key("rumahl.os.advanced_mode");
        let quota = key("rumahl.files.quota_bytes");
        let group = store.create_group("Staff").unwrap();

        // Device floor, group override, user override.
        store
            .set_assignment(
                PolicySubject::Device,
                quota.clone(),
                Some(PolicyValue::Number(16 * 1024 * 1024)),
            )
            .unwrap();
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            PolicyValue::Number(16 * 1024 * 1024)
        );
        store.update_members(&group.id, &[user()], &[]).unwrap();
        store
            .set_assignment(
                PolicySubject::Group(group.id),
                quota.clone(),
                Some(PolicyValue::Number(32 * 1024 * 1024)),
            )
            .unwrap();
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            PolicyValue::Number(32 * 1024 * 1024)
        );
        store
            .set_assignment(
                PolicySubject::User(user()),
                quota.clone(),
                Some(PolicyValue::Number(64 * 1024 * 1024)),
            )
            .unwrap();
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            PolicyValue::Number(64 * 1024 * 1024)
        );

        // Deleting the group removes its assignment; the user's own value
        // still applies, then the device floor, then the catalog default.
        store.delete_group(&group.id).unwrap();
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            PolicyValue::Number(64 * 1024 * 1024)
        );
        store
            .set_assignment(PolicySubject::User(user()), quota.clone(), None)
            .unwrap();
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            PolicyValue::Number(16 * 1024 * 1024)
        );

        // Defaults come from the catalog when nothing is assigned.
        store
            .set_assignment(PolicySubject::Device, quota.clone(), None)
            .unwrap();
        let default = policy_definition(&mode).unwrap().default;
        assert_eq!(store.effective(user(), &mode).unwrap(), default);
        assert_eq!(
            store.effective(user(), &quota).unwrap(),
            rumahl_core::PolicyValue::Number(rumahl_core::DEFAULT_PERSONAL_QUOTA_BYTES)
        );
        let _ = policy_catalog();
    }
}
