use crate::identity::Identity;

use super::{
    GrantId,
    PermissionGrant,
};

#[derive(Debug, Default)]
pub struct InMemoryGrantStore {
    grants: Vec<PermissionGrant>,
}

impl InMemoryGrantStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        grant: PermissionGrant,
    ) {
        self.grants.push(grant);
    }

    pub fn get(
        &self,
        id: &GrantId,
    ) -> Option<&PermissionGrant> {
        self.grants
            .iter()
            .find(|grant| grant.id() == id)
    }

    pub fn remove(
        &mut self,
        id: &GrantId,
    ) -> Option<PermissionGrant> {
        let position = self
            .grants
            .iter()
            .position(|grant| grant.id() == id)?;

        Some(self.grants.remove(position))
    }

    pub fn grants_for_subject(
        &self,
        subject: &Identity,
    ) -> Vec<&PermissionGrant> {
        self.grants
            .iter()
            .filter(|grant| grant.subject() == subject)
            .collect()
    }

    pub fn grants(&self) -> &[PermissionGrant] {
        &self.grants
    }

    pub fn len(&self) -> usize {
        self.grants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId,
        AppIdentity,
        InstallationId,
        PermissionId,
        PermissionScope,
        PublisherId,
        ResourceKey,
        ResourceKind,
        ResourceNamespace,
        ResourceRef,
    };

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn file(key: &str) -> ResourceRef {
        ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse(key).unwrap(),
        )
    }

    fn file_read_grant(
        app: &AppIdentity,
        key: &str,
    ) -> PermissionGrant {
        PermissionGrant::new(
            app.clone().into(),
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file(key)],
            app.clone().into(),
        )
        .unwrap()
    }

    #[test]
    fn inserts_grant() {
        let app = notes_app();
        let grant = file_read_grant(&app, "document-1");

        let mut store = InMemoryGrantStore::new();

        assert!(store.is_empty());

        store.insert(grant);

        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn gets_grant_by_id() {
        let app = notes_app();
        let grant = file_read_grant(&app, "document-1");

        let grant_id = *grant.id();

        let mut store = InMemoryGrantStore::new();
        store.insert(grant);

        let stored = store.get(&grant_id).unwrap();

        assert_eq!(stored.id(), &grant_id);
    }

    #[test]
    fn removes_grant_by_id() {
        let app = notes_app();
        let grant = file_read_grant(&app, "document-1");

        let grant_id = *grant.id();

        let mut store = InMemoryGrantStore::new();
        store.insert(grant);

        let removed = store.remove(&grant_id);

        assert!(removed.is_some());
        assert!(store.is_empty());
        assert!(store.get(&grant_id).is_none());
    }

    #[test]
    fn removing_unknown_grant_returns_none() {
        let mut store = InMemoryGrantStore::new();

        assert!(
            store.remove(&GrantId::new()).is_none()
        );
    }

    #[test]
    fn lists_only_grants_for_requested_subject() {
        let notes = notes_app();

        let other = AppIdentity::new(
            AppId::parse("com.example.reader").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.example").unwrap(),
        );

        let notes_identity = notes.clone().into();

        let mut store = InMemoryGrantStore::new();

        store.insert(
            file_read_grant(
                &notes,
                "notes-document",
            )
        );

        store.insert(
            file_read_grant(
                &other,
                "other-document",
            )
        );

        let grants =
            store.grants_for_subject(&notes_identity);

        assert_eq!(grants.len(), 1);

        assert_eq!(
            grants[0].subject(),
            &notes_identity
        );
    }
}