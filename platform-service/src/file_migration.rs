//! One-way migration of the personal store into each account's home.
//!
//! The home is the source of truth for user files (see
//! `HomePersonalFiles`). Existing installations still hold their documents in
//! the SQLite store, so this migration copies them into the home, seals them
//! again with the same key configuration, and translates every files grant
//! from the old row ids to the home ids. The store is never modified: it stays
//! the rollback until a later release removes it.
//!
//! Invariants:
//!
//! * Nothing is overwritten. A file or folder that already exists in the home
//!   is kept and reported; a grant pointing at the same path is remapped to the
//!   home entry, so the user's own documents win the merge.
//! * A grant whose resources cannot all be resolved is dropped, never widened.
//!   After the switch the store is no longer served, so an unresolvable file id
//!   can never be honoured again.
//! * The run is idempotent: re-running skips existing entries and keeps grants
//!   that already reference home ids.
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rumahl_core::{
    AccountStateRepository, AuditLog, Identity, InMemoryGrantStore, PermissionGrant, PersonalFiles,
    PlatformRecovery, PlatformSnapshot, PlatformSnapshotRepository, ResourceKind,
    ResourceNamespace, ResourceRef, UnixTimestamp, UserId,
};
use rumahl_persistence_sqlite::{
    HomePersonalFiles, SqliteAccountStateRepository, SqliteAuditLog, SqlitePersonalFiles,
    SqliteSnapshotRepository,
};

/// Where the migrated documents' id index lives.
const INDEX_DATABASE: &str = "files-index.sqlite";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileMigrationReport {
    /// Accounts that were visited.
    pub accounts: usize,
    /// Accounts without a home: their files stay in the store and their grants
    /// are dropped, because the store is no longer served after the switch.
    pub skipped: usize,
    /// Files copied into a home.
    pub files: usize,
    /// Folders created in a home.
    pub directories: usize,
    /// Entries already present in a home and kept unchanged.
    pub existing: usize,
    pub grants_remapped: usize,
    pub grants_dropped: usize,
}

#[derive(Debug)]
pub enum FileMigrationError {
    Store(rumahl_core::FileError),
    Accounts,
    Snapshot,
    Audit,
    StoreUnavailable,
    Index(String),
    TooDeep,
}

impl std::fmt::Display for FileMigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(f, "personal store read failed: {error:?}"),
            Self::Accounts => write!(f, "account state is unavailable"),
            Self::Snapshot => write!(f, "platform snapshot migration failed"),
            Self::Audit => write!(f, "audit log is unavailable"),
            Self::StoreUnavailable => write!(f, "personal store is unavailable"),
            Self::Index(error) => write!(f, "file index is unavailable: {error}"),
            Self::TooDeep => write!(f, "folder nesting is too deep to migrate safely"),
        }
    }
}

impl std::error::Error for FileMigrationError {}

/// Copies every account's stored documents into its home and remaps grants.
pub fn migrate_files(
    state_dir: &Path,
    home_root: &Path,
    key_provider: Option<Arc<dyn rumahl_persistence_sqlite::FieldKeyProvider>>,
) -> Result<FileMigrationReport, FileMigrationError> {
    let accounts = state_dir.join("accounts.sqlite");
    let store: Arc<dyn PersonalFiles> = match key_provider.clone() {
        Some(provider) => Arc::new(
            SqlitePersonalFiles::open_encrypted(state_dir.join("files.sqlite"), provider)
                .map_err(|_| FileMigrationError::StoreUnavailable)?,
        ),
        None => Arc::new(
            SqlitePersonalFiles::open(state_dir.join("files.sqlite"))
                .map_err(|_| FileMigrationError::StoreUnavailable)?,
        ),
    };
    let home = open_home(
        home_root,
        &accounts,
        &state_dir.join(INDEX_DATABASE),
        key_provider,
    )?;

    let state = SqliteAccountStateRepository::open(&accounts)
        .and_then(|repository| repository.load())
        .map_err(|_| FileMigrationError::Accounts)?;
    let mut report = FileMigrationReport::default();
    let mut migration = Migration::default();
    for account in state.accounts().accounts() {
        let owner = *account.user_id();
        report.accounts += 1;
        // A home that does not exist was never provisioned. Creating one here
        // would invent a location, so the account is reported instead and its
        // grants are dropped by the remapping step (fail closed).
        if home.list(owner, "root").is_err() {
            report.skipped += 1;
            continue;
        }
        walk(
            &store,
            &home,
            owner,
            "root",
            "root",
            0,
            &mut report,
            &mut migration,
        )?;
    }

    let (remapped, dropped) = remap_grants(state_dir, &home, &migration)?;
    report.grants_remapped = remapped;
    report.grants_dropped = dropped;

    // A durable record of the migration, independent of console output.
    let audit = SqliteAuditLog::open(state_dir.join("audit.sqlite"))
        .map_err(|_| FileMigrationError::Audit)?;
    let at = UnixTimestamp::now().unwrap_or_else(|_| UnixTimestamp::from_seconds(0));
    if let Ok(event) = rumahl_core::NewAuditEvent::new(
        at,
        rumahl_core::AuditActor::Os,
        rumahl_core::AuditAction::Maintenance,
        rumahl_core::AuditOutcome::Success,
    )
    .with_target("files-migrated")
    {
        let _ = audit.record(event);
    }

    Ok(report)
}

fn open_home(
    home_root: &Path,
    accounts: &Path,
    index: &Path,
    key_provider: Option<Arc<dyn rumahl_persistence_sqlite::FieldKeyProvider>>,
) -> Result<HomePersonalFiles, FileMigrationError> {
    let opened = match key_provider {
        Some(provider) => HomePersonalFiles::open_encrypted(home_root, accounts, index, provider),
        None => HomePersonalFiles::open(home_root, accounts, index),
    };
    opened.map_err(|error| FileMigrationError::Index(error.to_string()))
}

#[derive(Default)]
struct Migration {
    /// Every store file id seen, mapped to its home id when the copy or the
    /// merge produced one. Only these keys are old store ids.
    old_ids: HashMap<String, Option<String>>,
}

#[allow(clippy::too_many_arguments)]
fn walk(
    store: &Arc<dyn PersonalFiles>,
    home: &HomePersonalFiles,
    owner: UserId,
    parent: &str,
    home_parent: &str,
    depth: usize,
    report: &mut FileMigrationReport,
    migration: &mut Migration,
) -> Result<(), FileMigrationError> {
    /// Guards the recursion: the store has no depth limit of its own.
    const MAX_DEPTH: usize = 128;
    if depth > MAX_DEPTH {
        return Err(FileMigrationError::TooDeep);
    }
    let entries = store
        .list(owner, parent)
        .map_err(FileMigrationError::Store)?;
    for entry in entries {
        if entry.directory {
            match home.create(owner, home_parent, &entry.name, None) {
                Ok(()) => report.directories += 1,
                // Standard folders already exist in a provisioned home; the
                // store's folder merges into the user's own one.
                Err(_) => report.existing += 1,
            }
            let home_id = find_id(home, owner, home_parent, &entry.name)?;
            migration.old_ids.insert(entry.id.clone(), home_id.clone());
            if let Some(home_id) = home_id {
                walk(
                    store,
                    home,
                    owner,
                    &entry.id,
                    &home_id,
                    depth + 1,
                    report,
                    migration,
                )?;
            }
            continue;
        }
        let (_, content) = store
            .read(owner, &entry.id)
            .map_err(FileMigrationError::Store)?;
        match home.create(owner, home_parent, &entry.name, Some(content)) {
            Ok(()) => report.files += 1,
            // An existing file is kept: no data is ever overwritten.
            Err(_) => report.existing += 1,
        }
        let home_id = find_id(home, owner, home_parent, &entry.name)?;
        migration.old_ids.insert(entry.id, home_id);
    }
    Ok(())
}

fn find_id(
    home: &HomePersonalFiles,
    owner: UserId,
    parent: &str,
    name: &str,
) -> Result<Option<String>, FileMigrationError> {
    Ok(home
        .list(owner, parent)
        .map_err(FileMigrationError::Store)?
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.id))
}

/// Rewrites the file resources of every grant through the id mapping. A grant
/// with a resource that is neither a migrated store id nor an existing home id
/// is dropped, never widened.
fn remap_grants(
    state_dir: &Path,
    home: &HomePersonalFiles,
    migration: &Migration,
) -> Result<(usize, usize), FileMigrationError> {
    let repository = SqliteSnapshotRepository::open(state_dir.join("platform.sqlite"))
        .map_err(|_| FileMigrationError::Snapshot)?;
    let Some(snapshot) = repository
        .load()
        .map_err(|_| FileMigrationError::Snapshot)?
    else {
        return Ok((0, 0));
    };
    let mut state = rumahl_core::PlatformState::new();
    let mut grants = InMemoryGrantStore::new();
    PlatformRecovery::new()
        .recover(&snapshot, &mut state, &mut grants)
        .map_err(|_| FileMigrationError::Snapshot)?;

    let files = ResourceNamespace::parse("rumahl.files").ok();
    let file_kind = ResourceKind::parse("file").ok();
    let is_file = |resource: &ResourceRef| {
        Some(resource.namespace()) == files.as_ref() && Some(resource.kind()) == file_kind.as_ref()
    };

    let mut next = InMemoryGrantStore::new();
    let mut remapped = 0;
    let mut dropped = 0;
    for grant in grants.grants() {
        if !grant.resources().iter().any(&is_file) {
            next.insert(grant.clone());
            continue;
        }
        // Files consent is personal: the issuing user owns the documents it
        // names, so their home decides whether a non-store id is resolvable.
        let owner = match grant.granted_by() {
            Identity::User(user) => Some(*user.id()),
            Identity::App(_) | Identity::Service(_) => None,
        };
        let mut resolved = Vec::with_capacity(grant.resources().len());
        let mut complete = true;
        for resource in grant.resources() {
            if !is_file(resource) {
                resolved.push(resource.clone());
                continue;
            }
            let key = resource.key().as_str();
            match migration.old_ids.get(key) {
                // An old id that migrated: point at the home entry.
                Some(Some(home_id)) => match rumahl_core::ResourceKey::parse(home_id) {
                    Ok(key) => resolved.push(ResourceRef::new(
                        resource.namespace().clone(),
                        resource.kind().clone(),
                        key,
                    )),
                    Err(_) => complete = false,
                },
                // An old id whose file did not migrate: the consent cannot be
                // honoured, so the grant does not survive.
                Some(None) => complete = false,
                // Not an old id: it must already name a file in the owner's
                // home, otherwise the grant refers to something we cannot serve.
                None => match owner {
                    Some(owner) if home.contains(owner, key) => resolved.push(resource.clone()),
                    _ => complete = false,
                },
            }
        }
        if !complete {
            dropped += 1;
            continue;
        }
        let grant: PermissionGrant = grant
            .with_resources(resolved)
            .map_err(|_| FileMigrationError::Snapshot)?;
        next.insert(grant);
        remapped += 1;
    }
    repository
        .store(&PlatformSnapshot::capture(&state, &next))
        .map_err(|_| FileMigrationError::Snapshot)?;
    Ok((remapped, dropped))
}
