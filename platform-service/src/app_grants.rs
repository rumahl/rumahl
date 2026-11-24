//! User consent over app filesystem permissions (`rumahl.files.*`).
//!
//! The consent model, end to end:
//!
//! * An app's manifest *requests* `rumahl.files.read`/`rumahl.files.write`.
//!   The catalog validator already rejected unknown OS permissions at install
//!   time, so a request that survives installation is one the platform honors.
//! * The user *consents* through the shell: either durably (persisted with the
//!   platform snapshot, optionally with an expiry) or for the current browser
//!   session only. Consent is always explicit — it names the exact personal
//!   files (resource keys) the app may touch. Apps never issue grants; only
//!   the consenting user does.
//! * Every app file operation is authorized *per call* against the durable and
//!   session grants. Expired, session-mismatched or revoked grants are
//!   indistinguishable from missing ones. Durable file grants are personal:
//!   they apply only for the user who consented to them, never for another
//!   account sharing the installation.
//! * A restart grants nothing: session grants are memory-only and durable
//!   grants re-enter through the snapshot with their expiry intact.

use std::collections::HashMap;
use std::sync::Mutex;

use rumahl_core::{
    AccountStateRepository, AuthorizationDecision, AuthorizationDenyReason, AuthorizationEngine,
    AuthorizationRequest, GrantAuthority, GrantId, GrantIssuerPolicy, GrantValidity, Identity,
    InMemoryGrantStore, OperationContext, OsPermissionCatalogError, PermissionGrant, PermissionId,
    PermissionScope, PlatformRecovery, PlatformSnapshot, PlatformSnapshotRepository, ResourceKey,
    ResourceKind, ResourceNamespace, ResourceRef, UnixTimestamp, UserId, UserIdentity, UserRole,
    os_permission_catalog_validate,
};
use rumahl_persistence_sqlite::SqliteAccountRoles;
use rumahl_platform_web::{
    AppAccessError, AppFileAccess, AppFileAction, AppFileDeny, AppFileOutcome, AppGrantInfo,
    AppGrantState, AppPermissionRequestInfo, GrantIssue, GrantValidityChoice, ShellIdentity,
};

use crate::apps::PersistentApps;

/// How many grants a single browser session may hold. A session that hits the
/// cap must revoke or restart; consent is not a storage service.
const MAX_SESSION_GRANTS: usize = 64;
/// Resource keys one grant may cover. The picker dialog works on selections;
/// anything larger is a bulk export and not consent in any meaningful sense.
const MAX_GRANT_RESOURCES: usize = 64;
/// Upper bound for an app-side read, mirroring the personal-files body limit.
const MAX_READ_BYTES: u64 = 16 * 1024 * 1024;

/// Personal-file resource refs: `rumahl.files:file:<id>`.
fn file_resource(key: &str) -> Option<ResourceRef> {
    Some(ResourceRef::new(
        ResourceNamespace::parse("rumahl.files").ok()?,
        ResourceKind::parse("file").ok()?,
        ResourceKey::parse(key).ok()?,
    ))
}

fn permission_id(value: &str) -> Option<PermissionId> {
    PermissionId::parse(value).ok()
}

/// Grants issued for one browser session. Memory-only by design: the map dies
/// with the process, so a restart can never resurrect session consent.
#[derive(Default)]
pub struct SessionGrantStore {
    grants: Mutex<HashMap<rumahl_core::SessionId, Vec<PermissionGrant>>>,
}

impl SessionGrantStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a session grant. Refuses (false) when the session already holds
    /// its cap, so a chatty app cannot grow the map without bound.
    pub fn insert(&self, grant: PermissionGrant) -> bool {
        let session = match grant.validity() {
            GrantValidity::Session { session } => *session,
            GrantValidity::Durable { .. } => return false,
        };
        let mut grants = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let entry = grants.entry(session).or_default();
        if entry.len() >= MAX_SESSION_GRANTS {
            return false;
        }
        entry.push(grant);
        true
    }

    /// The session's grants, oldest first.
    pub fn grants_for(&self, session: rumahl_core::SessionId) -> Vec<PermissionGrant> {
        self.grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&session)
            .cloned()
            .unwrap_or_default()
    }

    /// Drops everything a session held. Called when the browser session ends,
    /// so the store never grows past the live sessions.
    pub fn remove_session(&self, session: rumahl_core::SessionId) -> usize {
        let mut grants = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        grants.remove(&session).map_or(0, |removed| removed.len())
    }

    /// Removes one of the session's grants. True when the grant belonged to
    /// this session and is gone now.
    pub fn remove(&self, session: rumahl_core::SessionId, id: &GrantId) -> bool {
        let mut grants = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = grants.get_mut(&session) else {
            return false;
        };
        let before = entry.len();
        entry.retain(|grant| grant.id() != id);
        entry.len() != before
    }
}

/// The home-relative part of an internal channel path, or `None` when the
/// path names another account or is not a home path at all.
fn home_relative<'a>(path: &'a str, username: &str) -> Option<&'a str> {
    if let Some(rest) = path.strip_prefix("/home/") {
        let (name, relative) = rest.split_once('/').unwrap_or((rest, ""));
        return (name == username).then_some(relative);
    }
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("rumahl://home") {
        let rest = &path["rumahl://home".len()..];
        if rest.is_empty() {
            return Some("");
        }
        return rest.strip_prefix('/');
    }
    None
}

/// Loads the platform snapshot and recovers state + durable grants.
fn recovered(
    snapshot: Option<PlatformSnapshot>,
) -> Option<(rumahl_core::PlatformState, InMemoryGrantStore)> {
    let snapshot = snapshot?;
    let mut state = rumahl_core::PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    PlatformRecovery::new()
        .recover(&snapshot, &mut state, &mut grants)
        .ok()?;
    Some((state, grants))
}

/// The installed app behind an installation id, or `None` when unknown.
fn installed_app(
    snapshot: Option<PlatformSnapshot>,
    installation: rumahl_core::InstallationId,
) -> Option<rumahl_core::InstalledAppSnapshot> {
    let snapshot = snapshot?;
    snapshot
        .installed_apps()
        .iter()
        .find(|app| *app.installation_id() == installation)
        .cloned()
}

/// Only grants the calling user consented to, for this app's identity.
fn personal_grants<'a>(
    grants: impl IntoIterator<Item = &'a PermissionGrant>,
    app: &rumahl_core::AppIdentity,
    user: UserId,
) -> Vec<PermissionGrant> {
    grants
        .into_iter()
        .filter(|grant| {
            grant.subject() == &Identity::App(app.clone())
                && matches!(grant.granted_by(), Identity::User(issuer) if issuer.id() == &user)
        })
        .cloned()
        .collect()
}

fn grant_info(grant: &PermissionGrant) -> AppGrantInfo {
    let (validity, expires_at) = match grant.validity() {
        GrantValidity::Durable { expires_at } => ("durable", expires_at.map(|t| t.as_seconds())),
        GrantValidity::Session { .. } => ("session", None),
    };
    AppGrantInfo {
        id: grant.id().to_string(),
        permission: grant.permission().as_str().to_owned(),
        scope: scope_name(grant.scope()).to_owned(),
        resources: grant
            .resources()
            .iter()
            .map(ResourceRef::to_string)
            .collect(),
        validity: validity.to_owned(),
        expires_at,
    }
}

fn scope_name(scope: PermissionScope) -> &'static str {
    match scope {
        PermissionScope::AppPrivate => "app-private",
        PermissionScope::UserOwn => "user-own",
        PermissionScope::UserSelected => "user-selected",
        PermissionScope::Explicit => "explicit",
        PermissionScope::FamilyShared => "family-shared",
        PermissionScope::System => "system",
    }
}

/// The user's role, for the issuer policy. Fails closed when the role store
/// is unavailable: consent without a known issuer role is refused.
fn issuer_role(
    roles: &Option<SqliteAccountRoles>,
    user: UserId,
) -> Result<UserRole, AppAccessError> {
    roles
        .as_ref()
        .ok_or(AppAccessError::Unavailable)?
        .role(user)
        .map_err(|_| AppAccessError::Unavailable)
}

impl PersistentApps {
    /// The consent state of one installed app as the calling user sees it.
    pub fn app_grant_state(
        &self,
        identity: ShellIdentity,
        installation: rumahl_core::InstallationId,
    ) -> Result<AppGrantState, AppAccessError> {
        self.authenticate(identity)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let app = installed_app(snapshot.clone(), installation).ok_or(AppAccessError::Denied)?;
        let requests = app
            .manifest()
            .permission_requests()
            .iter()
            .map(|request| AppPermissionRequestInfo {
                permission: request.permission().as_str().to_owned(),
                scope: scope_name(request.requested_scope()).to_owned(),
                required: request.required(),
                reason: request.reason().map(str::to_owned),
            })
            .collect();
        let (_, durable) = recovered(snapshot).ok_or(AppAccessError::Unavailable)?;
        let mut grants: Vec<AppGrantInfo> =
            personal_grants(durable.grants(), app.identity(), identity.user_id)
                .iter()
                .map(grant_info)
                .collect();
        grants.extend(
            self.session_grants
                .grants_for(identity.session_id)
                .iter()
                .filter(|grant| grant.subject() == &Identity::App(app.identity().clone()))
                .map(grant_info),
        );
        Ok(AppGrantState { requests, grants })
    }

    /// Issues a grant from an explicit user consent.
    pub fn issue_app_grant(
        &self,
        identity: ShellIdentity,
        installation: rumahl_core::InstallationId,
        issue: &GrantIssue,
    ) -> Result<AppGrantInfo, AppAccessError> {
        self.authenticate(identity)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let app = installed_app(snapshot.clone(), installation).ok_or(AppAccessError::Denied)?;
        let manifest = app.manifest().clone();

        // Only permissions the app actually asked for can be consented to,
        // and only in the form the OS catalog defines.
        let permission = permission_id(&issue.permission).ok_or(AppAccessError::Denied)?;
        let requested = manifest
            .permission_requests()
            .iter()
            .find(|request| request.permission() == &permission)
            .ok_or(AppAccessError::Denied)?;
        match os_permission_catalog_validate(requested) {
            Ok(()) => {}
            Err(OsPermissionCatalogError::UnknownPermission { .. })
            | Err(OsPermissionCatalogError::ScopeNotAllowed { .. }) => {
                return Err(AppAccessError::Denied);
            }
        }
        if requested.requested_scope() != PermissionScope::UserSelected
            && requested.requested_scope() != PermissionScope::Explicit
        {
            // The app asked for a scope the consent flow cannot materialize.
            return Err(AppAccessError::Denied);
        }

        // Consent is explicit: it names the exact files. Each key must exist
        // in the consenting user's personal files, so ids cannot be guessed
        // into a future file.
        if issue.resources.is_empty() || issue.resources.len() > MAX_GRANT_RESOURCES {
            return Err(AppAccessError::Denied);
        }
        let files = self.files.as_ref().ok_or(AppAccessError::Unavailable)?;
        let mut resources = Vec::with_capacity(issue.resources.len());
        for key in &issue.resources {
            let resource = file_resource(key).ok_or(AppAccessError::Denied)?;
            files
                .read(identity.user_id, key)
                .map_err(|_| AppAccessError::Denied)?;
            resources.push(resource);
        }

        let role = issuer_role(&self.roles, identity.user_id)?;
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(identity.user_id, role);

        let validity = match issue.validity {
            GrantValidityChoice::Session => GrantValidity::session(identity.session_id),
            GrantValidityChoice::Durable { expires_in } => {
                let now = UnixTimestamp::now().map_err(|_| AppAccessError::Unavailable)?;
                match expires_in {
                    // A consent that overflows the clock is refused, not clamped.
                    Some(seconds) => match now.as_seconds().checked_add(seconds) {
                        Some(at) => GrantValidity::durable_until(UnixTimestamp::from_seconds(at)),
                        None => return Err(AppAccessError::Denied),
                    },
                    None => GrantValidity::durable(),
                }
            }
        };

        let grant = GrantAuthority::new()
            .issue_with_validity(
                &policy,
                Identity::User(UserIdentity::new(identity.user_id)),
                Identity::App(app.identity().clone()),
                permission,
                PermissionScope::Explicit,
                resources,
                validity,
            )
            .map_err(|_| AppAccessError::Denied)?;

        match grant.validity() {
            GrantValidity::Session { .. } => {
                if !self.session_grants.insert(grant.clone()) {
                    return Err(AppAccessError::Unavailable);
                }
            }
            GrantValidity::Durable { .. } => {
                // Persist exactly the durable set: recovery keeps every other
                // grant, the new one joins them, session grants stay out.
                let (state, mut durable) =
                    recovered(snapshot).ok_or(AppAccessError::Unavailable)?;
                durable.insert(grant.clone());
                self.platform
                    .store(&PlatformSnapshot::capture(&state, &durable))
                    .map_err(|_| AppAccessError::Unavailable)?;
            }
        }

        Ok(grant_info(&grant))
    }

    /// Drops the session's memory-only grants when the browser session ends.
    pub fn end_session(&self, session: rumahl_core::SessionId) {
        self.session_grants.remove_session(session);
    }

    /// Revokes a grant the calling user issued, durable or in this session.
    pub fn revoke_app_grant(
        &self,
        identity: ShellIdentity,
        installation: rumahl_core::InstallationId,
        grant_id: &str,
    ) -> Result<(), AppAccessError> {
        self.authenticate(identity)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let app = installed_app(snapshot.clone(), installation).ok_or(AppAccessError::Denied)?;
        let id = GrantId::parse(grant_id).map_err(|_| AppAccessError::Denied)?;

        // Session grants live in this session only; revoking one never
        // touches the snapshot.
        if self.session_grants.remove(identity.session_id, &id) {
            return Ok(());
        }

        let (state, mut durable) = recovered(snapshot).ok_or(AppAccessError::Unavailable)?;
        let mine = personal_grants(durable.grants(), app.identity(), identity.user_id);
        if !mine.iter().any(|grant| grant.id() == &id) {
            return Err(AppAccessError::Denied);
        }
        durable.remove(&id);
        self.platform
            .store(&PlatformSnapshot::capture(&state, &durable))
            .map_err(|_| AppAccessError::Unavailable)?;
        Ok(())
    }

    /// Resolves an internal path for a channel app to the caller's personal
    /// store id. Only `/home/<caller>/...` and `rumahl://home/...` are
    /// accepted; the grant on the returned id still decides the access.
    pub(crate) fn resolve_channel_file(
        &self,
        identity: ShellIdentity,
        installation: rumahl_core::InstallationId,
        path: &str,
    ) -> Result<String, AppAccessError> {
        self.authenticate(identity)?;
        self.app(identity, installation)?;
        let state = self
            .accounts
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let username = state
            .accounts()
            .get(&identity.user_id)
            .ok_or(AppAccessError::Denied)?
            .username()
            .as_str()
            .to_owned();
        let relative = home_relative(path, &username).ok_or(AppAccessError::Denied)?;
        let files = self.files.as_ref().ok_or(AppAccessError::Unavailable)?;
        files
            .resolve_path(identity.user_id, relative)
            .map(|(id, _)| id)
            .map_err(|error| match error {
                rumahl_core::FileError::Missing | rumahl_core::FileError::Invalid => {
                    AppAccessError::Denied
                }
                _ => AppAccessError::Unavailable,
            })
    }

    /// Reads or writes one personal file on an app's behalf, authorized per
    /// call against the app's durable and session grants.
    pub fn app_file_access(
        &self,
        identity: ShellIdentity,
        installation: rumahl_core::InstallationId,
        access: &AppFileAccess,
    ) -> Result<AppFileOutcome, AppAccessError> {
        self.authenticate(identity)?;
        let now = UnixTimestamp::now().map_err(|_| AppAccessError::Unavailable)?;
        let snapshot = self
            .platform
            .load()
            .map_err(|_| AppAccessError::Unavailable)?;
        let app = installed_app(snapshot.clone(), installation).ok_or(AppAccessError::Denied)?;
        let files = self.files.as_ref().ok_or(AppAccessError::Unavailable)?;

        let permission = match access.action {
            AppFileAction::Read => permission_id("rumahl.files.read"),
            AppFileAction::Write { .. } => permission_id("rumahl.files.write"),
        }
        .ok_or(AppAccessError::Unavailable)?;
        let resource = file_resource(&access.key).ok_or(AppAccessError::Denied)?;

        // Durable grants are personal: only the ones this user consented to
        // apply here, next to this session's grants.
        let (_, durable) = recovered(snapshot).ok_or(AppAccessError::Unavailable)?;
        let mut grants = personal_grants(durable.grants(), app.identity(), identity.user_id);
        grants.extend(
            self.session_grants
                .grants_for(identity.session_id)
                .into_iter()
                .filter(|grant| grant.subject() == &Identity::App(app.identity().clone())),
        );

        let context = OperationContext::for_app_as_user(
            app.identity().clone(),
            identity.user_id,
            identity.session_id,
        );
        let request = AuthorizationRequest::new(context, permission.clone(), Some(resource));
        let decision = AuthorizationEngine::new().authorize_at(&request, &grants, now);

        match decision {
            AuthorizationDecision::Allow => match &access.action {
                AppFileAction::Read => match files.read(identity.user_id, &access.key) {
                    Ok((name, bytes)) => {
                        if bytes.len() as u64 > MAX_READ_BYTES {
                            return Ok(AppFileOutcome::Missing);
                        }
                        Ok(AppFileOutcome::Content { name, bytes })
                    }
                    Err(rumahl_core::FileError::Missing) => Ok(AppFileOutcome::Missing),
                    Err(rumahl_core::FileError::Invalid) => Err(AppAccessError::Denied),
                    Err(_) => Err(AppAccessError::Unavailable),
                },
                AppFileAction::Write { bytes } => {
                    match files.write(identity.user_id, &access.key, bytes.clone()) {
                        Ok(()) => {
                            let name = files
                                .read(identity.user_id, &access.key)
                                .map(|(name, _)| name)
                                .unwrap_or_default();
                            Ok(AppFileOutcome::Allowed {
                                name,
                                size: bytes.len() as u64,
                            })
                        }
                        Err(rumahl_core::FileError::Missing) => Ok(AppFileOutcome::Missing),
                        Err(rumahl_core::FileError::Invalid) => Err(AppAccessError::Denied),
                        Err(_) => Err(AppAccessError::Unavailable),
                    }
                }
            },
            AuthorizationDecision::RequireUserApproval
            | AuthorizationDecision::RequireReauthentication => {
                // Nothing issues these today; they can only mean "no".
                Ok(AppFileOutcome::Denied(AppFileDeny::NoGrant))
            }
            AuthorizationDecision::Deny(reason) => Ok(AppFileOutcome::Denied(deny_reason(
                reason,
                app.manifest(),
                &permission,
            ))),
        }
    }
}

/// Maps an engine denial to the reason the shell needs: when the app asked
/// for the permission but no usable grant covers the file, the shell can
/// offer consent; otherwise access is simply refused.
fn deny_reason(
    reason: AuthorizationDenyReason,
    manifest: &rumahl_core::AppManifest,
    permission: &PermissionId,
) -> AppFileDeny {
    let requested = manifest
        .permission_requests()
        .iter()
        .any(|request| request.permission() == permission);
    match reason {
        AuthorizationDenyReason::NoGrantForActor
        | AuthorizationDenyReason::PermissionNotGranted => {
            if requested {
                AppFileDeny::ApprovalRequired
            } else {
                AppFileDeny::NoGrant
            }
        }
        AuthorizationDenyReason::ResourceRequired
        | AuthorizationDenyReason::ResourceOutsideScope => AppFileDeny::OutsideScope,
        AuthorizationDenyReason::UnsupportedScope => AppFileDeny::NoGrant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rumahl_core::{
        AppId, AppIdentity, InstallationId, PermissionRequest, PublisherId, SessionId,
    };

    fn file_grant(session: Option<SessionId>) -> PermissionGrant {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );
        let user = UserId::new();
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(user, UserRole::User);
        let validity = match session {
            Some(session) => GrantValidity::session(session),
            None => GrantValidity::durable(),
        };
        GrantAuthority::new()
            .issue_with_validity(
                &policy,
                Identity::User(UserIdentity::new(user)),
                Identity::App(app),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![file_resource("abc123").unwrap()],
                validity,
            )
            .unwrap()
    }

    #[test]
    fn session_store_keeps_grants_per_session() {
        let store = SessionGrantStore::new();
        let session = SessionId::new();
        assert!(store.insert(file_grant(Some(session))));

        assert_eq!(store.grants_for(session).len(), 1);
        assert!(store.grants_for(SessionId::new()).is_empty());
    }

    #[test]
    fn session_store_refuses_durable_grants_and_duplicates_beyond_the_cap() {
        let store = SessionGrantStore::new();
        assert!(!store.insert(file_grant(None)));

        let session = SessionId::new();
        for _ in 0..MAX_SESSION_GRANTS {
            assert!(store.insert(file_grant(Some(session))));
        }
        assert!(!store.insert(file_grant(Some(session))));
    }

    #[test]
    fn ending_a_session_drops_its_grants() {
        let store = SessionGrantStore::new();
        let session = SessionId::new();
        let other = SessionId::new();
        store.insert(file_grant(Some(session)));
        store.insert(file_grant(Some(other)));

        assert_eq!(store.remove_session(session), 1);

        assert!(store.grants_for(session).is_empty());
        assert_eq!(store.grants_for(other).len(), 1);
        assert_eq!(store.remove_session(session), 0);
    }

    #[test]
    fn session_store_removes_only_its_own_grants() {
        let store = SessionGrantStore::new();
        let session = SessionId::new();
        let grant = file_grant(Some(session));
        let id = *grant.id();
        store.insert(grant);

        assert!(!store.remove(SessionId::new(), &id));
        assert!(store.remove(session, &id));
        assert!(store.grants_for(session).is_empty());
    }

    #[test]
    fn personal_grants_are_scoped_to_the_consenting_user() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );
        let user = UserId::new();
        let mut policy = GrantIssuerPolicy::new();
        policy.set_user_role(user, UserRole::User);
        let mine = GrantAuthority::new()
            .issue(
                &policy,
                Identity::User(UserIdentity::new(user)),
                Identity::App(app.clone()),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![file_resource("document-1").unwrap()],
            )
            .unwrap();
        let other_user = UserId::new();
        let mut other_policy = GrantIssuerPolicy::new();
        other_policy.set_user_role(other_user, UserRole::User);
        let theirs = GrantAuthority::new()
            .issue(
                &other_policy,
                Identity::User(UserIdentity::new(other_user)),
                Identity::App(app.clone()),
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::Explicit,
                vec![file_resource("document-2").unwrap()],
            )
            .unwrap();

        let kept = personal_grants([&mine, &theirs], &app, user);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id(), mine.id());
    }

    #[test]
    fn file_resources_only_accept_valid_resource_keys() {
        assert!(file_resource("0123456789abcdef0123456789abcdef").is_some());
        assert!(file_resource("root").is_some());
        // Path-like and empty ids are not resource keys.
        assert!(file_resource("../etc/passwd").is_none());
        assert!(file_resource("with space").is_none());
        assert!(file_resource("").is_none());
    }

    #[test]
    fn deny_reason_separates_missing_consent_from_out_of_scope() {
        let mut manifest = rumahl_core::AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            rumahl_core::AppVersion::new(1, 0, 0),
            "Notes",
            rumahl_core::RuntimeDescriptor::web(),
        )
        .unwrap();
        manifest
            .add_permission_request(PermissionRequest::new(
                PermissionId::parse("rumahl.files.read").unwrap(),
                PermissionScope::UserSelected,
                false,
                None,
            ))
            .unwrap();

        let permission = PermissionId::parse("rumahl.files.read").unwrap();
        assert_eq!(
            deny_reason(
                AuthorizationDenyReason::PermissionNotGranted,
                &manifest,
                &permission
            ),
            AppFileDeny::ApprovalRequired
        );
        assert_eq!(
            deny_reason(
                AuthorizationDenyReason::ResourceOutsideScope,
                &manifest,
                &permission
            ),
            AppFileDeny::OutsideScope
        );

        let bare = rumahl_core::AppManifest::new(
            AppId::parse("com.rumahl.bare").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            rumahl_core::AppVersion::new(1, 0, 0),
            "Bare",
            rumahl_core::RuntimeDescriptor::web(),
        )
        .unwrap();
        assert_eq!(
            deny_reason(
                AuthorizationDenyReason::PermissionNotGranted,
                &bare,
                &permission
            ),
            AppFileDeny::NoGrant
        );
    }
}
