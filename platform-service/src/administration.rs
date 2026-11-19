//! Persistent account and session administration for the local service.
use std::sync::Arc;

use rumahl_account_auth::LocalAccountAdministrationService;
use rumahl_core::{
    AccountSession, AccountStateRepository, AccountStatus, AuditLog, AuditQuery, UnixTimestamp,
    UserId, UserRole,
};
use rumahl_persistence_sqlite::{
    BrowserSessionError, SqliteAccountRoles, SqliteAccountStateRepository,
    SqliteBrowserSessionRepository, SqliteLocalAccountAdministrationRepository,
};
use rumahl_platform_web::{AdminAuditEntry, AdminError, AdminSession, AdminUser, Administration};
use rumahl_ui_contracts::ShellUserRole;
use std::path::Path;

use crate::{LocalPasswordBlocklist, ServiceError, shell_role};

type PasswordAdministration = LocalAccountAdministrationService<
    SqliteLocalAccountAdministrationRepository,
    LocalPasswordBlocklist,
>;

pub struct LocalAdministration {
    accounts: SqliteAccountStateRepository,
    roles: SqliteAccountRoles,
    passwords: PasswordAdministration,
    sessions: SqliteBrowserSessionRepository,
    audit: Option<Arc<dyn AuditLog>>,
}

impl LocalAdministration {
    pub fn open(path: &Path, audit: Option<Arc<dyn AuditLog>>) -> Result<Self, ServiceError> {
        let roles = SqliteAccountRoles::open(path)?;
        roles.ensure_owner()?;
        Ok(Self {
            accounts: SqliteAccountStateRepository::open(path)?,
            roles,
            passwords: LocalAccountAdministrationService::new(
                SqliteLocalAccountAdministrationRepository::open(path)?,
                LocalPasswordBlocklist::default(),
            ),
            sessions: SqliteBrowserSessionRepository::open(path)?,
            audit,
        })
    }

    fn require_admin(&self, actor: UserId) -> Result<(), AdminError> {
        if self
            .roles
            .role(actor)
            .map_err(|_| AdminError::Unavailable)?
            .is_administrator()
        {
            Ok(())
        } else {
            Err(AdminError::Forbidden)
        }
    }

    fn admin_user(&self, account: &rumahl_core::LocalAccount, role: UserRole) -> AdminUser {
        AdminUser {
            id: account.user_id().to_string(),
            username: account.username().as_str().to_owned(),
            display_name: account.display_name().to_owned(),
            status: status_name(account.status()).to_owned(),
            role: shell_role(role),
        }
    }

    fn administrator_count(&self, state: &rumahl_core::AccountState) -> Result<usize, AdminError> {
        let roles = self.roles.roles().map_err(|_| AdminError::Unavailable)?;
        let mut count = 0;
        for (user_id, role) in roles {
            if role.is_administrator()
                && state
                    .accounts()
                    .get(&user_id)
                    .is_some_and(rumahl_core::LocalAccount::can_authenticate)
            {
                count += 1;
            }
        }
        Ok(count)
    }
}

impl Administration for LocalAdministration {
    fn role(&self, user_id: UserId) -> Result<UserRole, AdminError> {
        self.roles
            .role(user_id)
            .map_err(|_| AdminError::Unavailable)
    }

    fn list_users(&self, actor: UserId) -> Result<Vec<AdminUser>, AdminError> {
        self.require_admin(actor)?;
        let state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        let roles = self.roles.roles().map_err(|_| AdminError::Unavailable)?;
        let mut users = state
            .accounts()
            .accounts()
            .iter()
            .map(|account| {
                let role = roles
                    .iter()
                    .find(|(user_id, _)| user_id == account.user_id())
                    .map_or(UserRole::User, |(_, role)| *role);
                self.admin_user(account, role)
            })
            .collect::<Vec<_>>();
        users.sort_by(|left, right| left.username.cmp(&right.username));
        Ok(users)
    }

    fn create_user(
        &self,
        actor: UserId,
        username: &str,
        display_name: &str,
        password: String,
        role: ShellUserRole,
    ) -> Result<AdminUser, AdminError> {
        self.require_admin(actor)?;
        if role == ShellUserRole::Owner {
            return Err(AdminError::Invalid);
        }
        let mut state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        let user_id = self
            .passwords
            .provision_password_account(
                &mut state,
                username,
                display_name,
                password,
                UnixTimestamp::now().map_err(|_| AdminError::Unavailable)?,
            )
            .map_err(|_| AdminError::Invalid)?;
        let core_role = core_role(role);
        self.roles
            .set_role(user_id, core_role)
            .map_err(|_| AdminError::Unavailable)?;
        let account = state
            .accounts()
            .get(&user_id)
            .ok_or(AdminError::Unavailable)?;
        Ok(self.admin_user(account, core_role))
    }

    fn update_user(
        &self,
        actor: UserId,
        user_id: UserId,
        status: Option<String>,
        role: Option<ShellUserRole>,
        username: Option<String>,
        display_name: Option<String>,
    ) -> Result<AdminUser, AdminError> {
        self.require_admin(actor)?;
        let mut state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        let current = state.accounts().get(&user_id).ok_or(AdminError::NotFound)?;
        let current_role = self
            .roles
            .role(user_id)
            .map_err(|_| AdminError::Unavailable)?;
        let current_status = current.status();
        let current_username = current.username().as_str().to_owned();
        let current_display_name = current.display_name().to_owned();
        let next_role = role.map(core_role).unwrap_or(current_role);
        if (next_role == UserRole::Owner) != (current_role == UserRole::Owner) {
            return Err(AdminError::Conflict);
        }
        let next_username = username.unwrap_or_else(|| current_username.clone());
        let next_display_name = display_name.unwrap_or_else(|| current_display_name.clone());
        let profile_changed =
            next_username != current_username || next_display_name != current_display_name;
        if profile_changed {
            if state.accounts().accounts().iter().any(|account| {
                account.user_id() != &user_id
                    && account
                        .username()
                        .as_str()
                        .eq_ignore_ascii_case(next_username.trim())
            }) {
                return Err(AdminError::Conflict);
            }
            state
                .update_account_profile(&user_id, &next_username, &next_display_name)
                .map_err(|_| AdminError::Invalid)?;
        }
        let next_status = status
            .as_deref()
            .map(parse_status)
            .transpose()?
            .unwrap_or(current_status);

        if user_id == actor
            && (next_status != AccountStatus::Active || !next_role.is_administrator())
        {
            return Err(AdminError::Conflict);
        }
        if current_role.is_administrator()
            && !next_role.is_administrator()
            && self.administrator_count(&state)? <= 1
        {
            return Err(AdminError::Conflict);
        }

        let now = UnixTimestamp::now().map_err(|_| AdminError::Unavailable)?;
        match next_status {
            AccountStatus::Active => match current_status {
                AccountStatus::Active => {}
                AccountStatus::Locked => state
                    .unlock_account(&user_id)
                    .map_err(|_| AdminError::Conflict)?,
                AccountStatus::Disabled => state
                    .enable_account(&user_id)
                    .map_err(|_| AdminError::Conflict)?,
            },
            AccountStatus::Locked => {
                state
                    .lock_account(&user_id, now)
                    .map_err(|_| AdminError::Conflict)?;
            }
            AccountStatus::Disabled => {
                state
                    .disable_account(&user_id, now)
                    .map_err(|_| AdminError::Conflict)?;
            }
        }
        if next_status != current_status || profile_changed {
            self.accounts
                .store(&state)
                .map_err(|_| AdminError::Unavailable)?;
        }
        if next_role != current_role {
            self.roles
                .set_role(user_id, next_role)
                .map_err(|_| AdminError::Unavailable)?;
        }
        let account = state
            .accounts()
            .get(&user_id)
            .ok_or(AdminError::Unavailable)?;
        Ok(self.admin_user(account, next_role))
    }

    fn delete_user(&self, actor: UserId, user_id: UserId) -> Result<(), AdminError> {
        self.require_admin(actor)?;
        if actor == user_id {
            return Err(AdminError::Conflict);
        }
        let state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        if state.accounts().get(&user_id).is_none() {
            return Err(AdminError::NotFound);
        }
        let actor_role = self
            .roles
            .role(actor)
            .map_err(|_| AdminError::Unavailable)?;
        let target_role = self
            .roles
            .role(user_id)
            .map_err(|_| AdminError::Unavailable)?;
        if target_role == UserRole::Owner
            || (target_role == UserRole::Administrator && actor_role != UserRole::Owner)
        {
            return Err(AdminError::Conflict);
        }
        if !self
            .accounts
            .delete_account(&user_id)
            .map_err(|_| AdminError::Unavailable)?
        {
            return Err(AdminError::NotFound);
        }
        Ok(())
    }

    fn reset_password(
        &self,
        actor: UserId,
        user_id: UserId,
        password: String,
    ) -> Result<(), AdminError> {
        self.require_admin(actor)?;
        let mut state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        self.passwords
            .reset_password(
                &mut state,
                user_id,
                password,
                UnixTimestamp::now().map_err(|_| AdminError::Unavailable)?,
            )
            .map_err(|_| AdminError::Invalid)?;
        self.accounts
            .store(&state)
            .map_err(|_| AdminError::Unavailable)?;
        Ok(())
    }

    fn list_sessions(&self, actor: UserId) -> Result<Vec<AdminSession>, AdminError> {
        self.require_admin(actor)?;
        let state = self.accounts.load().map_err(|_| AdminError::Unavailable)?;
        let now = UnixTimestamp::now().map_err(|_| AdminError::Unavailable)?;
        let mut sessions = state
            .sessions()
            .sessions()
            .iter()
            .map(|session| session_view(&state, session, now))
            .collect::<Result<Vec<_>, _>>()?;
        sessions.sort_by_key(|session| std::cmp::Reverse(session.last_seen_at));
        Ok(sessions)
    }

    fn revoke_session(
        &self,
        actor: UserId,
        session_id: rumahl_core::SessionId,
    ) -> Result<(), AdminError> {
        self.require_admin(actor)?;
        let now = UnixTimestamp::now().map_err(|_| AdminError::Unavailable)?;
        match self.sessions.revoke_by_id(session_id, now) {
            Ok(true) => Ok(()),
            Ok(false) => Err(AdminError::NotFound),
            Err(BrowserSessionError::InvalidProof | BrowserSessionError::Storage) => {
                Err(AdminError::Unavailable)
            }
        }
    }

    fn audit(
        &self,
        actor: UserId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AdminAuditEntry>, AdminError> {
        self.require_admin(actor)?;
        let Some(audit) = &self.audit else {
            return Ok(Vec::new());
        };
        audit
            .query_system(&AuditQuery {
                limit,
                offset,
                ..AuditQuery::default()
            })
            .map(|events| {
                events
                    .into_iter()
                    .map(|event| AdminAuditEntry {
                        sequence: event.sequence,
                        at: event.at.as_seconds(),
                        actor: event.actor.encode(),
                        action: event.action.as_str().to_owned(),
                        outcome: event.outcome.as_str().to_owned(),
                        target: event.target,
                    })
                    .collect()
            })
            .map_err(|_| AdminError::Unavailable)
    }
}

fn session_view(
    state: &rumahl_core::AccountState,
    session: &AccountSession,
    now: UnixTimestamp,
) -> Result<AdminSession, AdminError> {
    let account = state
        .accounts()
        .get(session.user_id())
        .ok_or(AdminError::Unavailable)?;
    Ok(AdminSession {
        session_id: session.session_id().to_string(),
        user_id: session.user_id().to_string(),
        username: account.username().as_str().to_owned(),
        display_name: account.display_name().to_owned(),
        authenticated_at: session.authenticated_at().as_seconds(),
        last_seen_at: session.last_seen_at().as_seconds(),
        expires_at: session.expires_at().as_seconds(),
        revoked_at: session.revoked_at().map(|at| at.as_seconds()),
        active: session.is_active_at(now),
    })
}

fn status_name(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Locked => "locked",
        AccountStatus::Disabled => "disabled",
    }
}

fn parse_status(value: &str) -> Result<AccountStatus, AdminError> {
    match value {
        "active" => Ok(AccountStatus::Active),
        "locked" => Ok(AccountStatus::Locked),
        "disabled" => Ok(AccountStatus::Disabled),
        _ => Err(AdminError::Invalid),
    }
}

fn core_role(role: ShellUserRole) -> UserRole {
    match role {
        ShellUserRole::User => UserRole::User,
        ShellUserRole::Manager => UserRole::Manager,
        ShellUserRole::Administrator => UserRole::Administrator,
        ShellUserRole::Owner => UserRole::Owner,
    }
}
