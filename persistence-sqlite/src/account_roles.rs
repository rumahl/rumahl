//! Persistent account roles. Account state remains authentication-focused;
//! authorization roles live beside it and are read by the shell snapshot.
use rumahl_core::{UserId, UserRole};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{error::Error, fmt, path::Path, sync::Mutex, time::Duration};

use crate::identity_schema;

#[derive(Debug)]
pub enum AccountRoleError {
    Storage,
    UnknownUser,
    LockPoisoned,
}

impl fmt::Display for AccountRoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("account role operation failed")
    }
}

impl Error for AccountRoleError {}

pub struct SqliteAccountRoles {
    connection: Mutex<Connection>,
}

impl SqliteAccountRoles {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AccountRoleError> {
        Self::from_connection(Connection::open(path).map_err(|_| AccountRoleError::Storage)?)
    }

    pub fn open_in_memory() -> Result<Self, AccountRoleError> {
        Self::from_connection(Connection::open_in_memory().map_err(|_| AccountRoleError::Storage)?)
    }

    fn from_connection(connection: Connection) -> Result<Self, AccountRoleError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| AccountRoleError::Storage)?;
        identity_schema::initialize(&connection).map_err(|_| AccountRoleError::Storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// The first installation has no role rows yet. Promote its first account
    /// once so an administrator exists after upgrading an older state volume.
    pub fn ensure_owner(&self) -> Result<(), AccountRoleError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AccountRoleError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| AccountRoleError::Storage)?;
        let assigned: bool = transaction
            .query_row("SELECT EXISTS(SELECT 1 FROM account_role)", [], |row| {
                row.get(0)
            })
            .map_err(|_| AccountRoleError::Storage)?;
        if !assigned {
            let first: Option<String> = transaction
                .query_row(
                    "SELECT user_id FROM local_account ORDER BY username LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|_| AccountRoleError::Storage)?;
            if let Some(user_id) = first {
                transaction
                    .execute(
                        "INSERT INTO account_role (user_id, role) VALUES (?1, 'owner')",
                        [user_id],
                    )
                    .map_err(|_| AccountRoleError::Storage)?;
            }
        }
        transaction.commit().map_err(|_| AccountRoleError::Storage)
    }

    pub fn role(&self, user_id: UserId) -> Result<UserRole, AccountRoleError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AccountRoleError::LockPoisoned)?;
        let role: Option<String> = connection
            .query_row(
                "SELECT role FROM account_role WHERE user_id = ?1",
                [user_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| AccountRoleError::Storage)?;
        role.as_deref()
            .and_then(UserRole::parse)
            .map_or(Ok(UserRole::User), Ok)
    }

    pub fn roles(&self) -> Result<Vec<(UserId, UserRole)>, AccountRoleError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AccountRoleError::LockPoisoned)?;
        let mut statement = connection
            .prepare("SELECT user_id, role FROM account_role ORDER BY user_id")
            .map_err(|_| AccountRoleError::Storage)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| AccountRoleError::Storage)?;
        let mut roles = Vec::new();
        for row in rows {
            let (user_id, role) = row.map_err(|_| AccountRoleError::Storage)?;
            let user_id = UserId::parse(&user_id).map_err(|_| AccountRoleError::Storage)?;
            let role = UserRole::parse(&role).ok_or(AccountRoleError::Storage)?;
            roles.push((user_id, role));
        }
        Ok(roles)
    }

    pub fn set_role(&self, user_id: UserId, role: UserRole) -> Result<(), AccountRoleError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AccountRoleError::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| AccountRoleError::Storage)?;
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM local_account WHERE user_id = ?1)",
                [user_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| AccountRoleError::Storage)?;
        if !exists {
            return Err(AccountRoleError::UnknownUser);
        }
        transaction
            .execute(
                "INSERT INTO account_role (user_id, role) VALUES (?1, ?2)
                 ON CONFLICT(user_id) DO UPDATE SET role = excluded.role",
                params![user_id.to_string(), role.as_str()],
            )
            .map_err(|_| AccountRoleError::Storage)?;
        transaction.commit().map_err(|_| AccountRoleError::Storage)
    }
}
