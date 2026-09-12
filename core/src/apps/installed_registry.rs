use std::error::Error;
use std::fmt;

use crate::{AppId, InstallationId};

use super::InstalledApp;

#[derive(Debug, Default, Clone)]
pub struct InstalledAppRegistry {
    apps: Vec<InstalledApp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstalledAppRegistryError {
    AppAlreadyInstalled,
    InstallationAlreadyRegistered,
}

impl InstalledAppRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(&self, app: &InstalledApp) -> Result<(), InstalledAppRegistryError> {
        if self
            .apps
            .iter()
            .any(|existing| existing.identity().app_id() == app.identity().app_id())
        {
            return Err(InstalledAppRegistryError::AppAlreadyInstalled);
        }

        if self
            .apps
            .iter()
            .any(|existing| existing.installation_id() == app.installation_id())
        {
            return Err(InstalledAppRegistryError::InstallationAlreadyRegistered);
        }

        Ok(())
    }

    pub(crate) fn register(&mut self, app: InstalledApp) -> Result<(), InstalledAppRegistryError> {
        self.can_register(&app)?;

        self.apps.push(app);

        Ok(())
    }

    pub fn get_by_app_id(&self, app_id: &AppId) -> Option<&InstalledApp> {
        self.apps
            .iter()
            .find(|app| app.identity().app_id() == app_id)
    }

    pub fn get_by_installation_id(
        &self,
        installation_id: &InstallationId,
    ) -> Option<&InstalledApp> {
        self.apps
            .iter()
            .find(|app| app.installation_id() == installation_id)
    }

    pub(crate) fn remove(&mut self, installation_id: &InstallationId) -> Option<InstalledApp> {
        let index = self
            .apps
            .iter()
            .position(|app| app.installation_id() == installation_id)?;

        Some(self.apps.remove(index))
    }

    pub fn apps(&self) -> &[InstalledApp] {
        &self.apps
    }

    pub fn len(&self) -> usize {
        self.apps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }
}

impl fmt::Display for InstalledAppRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppAlreadyInstalled => {
                write!(f, "app is already installed")
            }

            Self::InstallationAlreadyRegistered => {
                write!(f, "app installation is already registered")
            }
        }
    }
}

impl Error for InstalledAppRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppManifest, AppManifestValidator, AppVersion, PublisherId};

    fn installed_app(id: &str) -> InstalledApp {
        let manifest = AppManifest::new(
            AppId::parse(id).unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Test App",
        )
        .unwrap();

        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn registry_starts_empty() {
        let registry = InstalledAppRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registers_installed_app() {
        let app = installed_app("com.rumahl.notes");

        let mut registry = InstalledAppRegistry::new();

        registry.register(app).unwrap();

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn can_register_without_mutating_registry() {
        let app = installed_app("com.rumahl.notes");

        let registry = InstalledAppRegistry::new();

        assert!(registry.can_register(&app).is_ok());

        assert!(registry.is_empty());
    }

    #[test]
    fn rejects_second_installation_of_same_app() {
        let first = installed_app("com.rumahl.notes");

        let second = installed_app("com.rumahl.notes");

        assert_ne!(first.installation_id(), second.installation_id());

        let mut registry = InstalledAppRegistry::new();

        registry.register(first).unwrap();

        assert_eq!(
            registry.register(second).unwrap_err(),
            InstalledAppRegistryError::AppAlreadyInstalled
        );
    }

    #[test]
    fn resolves_app_by_app_id() {
        let app = installed_app("com.rumahl.notes");

        let app_id = app.identity().app_id().clone();

        let mut registry = InstalledAppRegistry::new();

        registry.register(app).unwrap();

        let resolved = registry.get_by_app_id(&app_id).unwrap();

        assert_eq!(resolved.identity().app_id(), &app_id);
    }

    #[test]
    fn resolves_app_by_installation_id() {
        let app = installed_app("com.rumahl.notes");

        let installation_id = *app.installation_id();

        let mut registry = InstalledAppRegistry::new();

        registry.register(app).unwrap();

        let resolved = registry.get_by_installation_id(&installation_id).unwrap();

        assert_eq!(resolved.installation_id(), &installation_id);
    }

    #[test]
    fn removes_exact_installation() {
        let notes = installed_app("com.rumahl.notes");

        let files = installed_app("com.rumahl.files");

        let notes_installation = *notes.installation_id();

        let files_installation = *files.installation_id();

        let mut registry = InstalledAppRegistry::new();

        registry.register(notes).unwrap();

        registry.register(files).unwrap();

        let removed = registry.remove(&notes_installation).unwrap();

        assert_eq!(removed.installation_id(), &notes_installation);

        assert!(
            registry
                .get_by_installation_id(&notes_installation)
                .is_none()
        );

        assert!(
            registry
                .get_by_installation_id(&files_installation)
                .is_some()
        );

        assert_eq!(registry.len(), 1);
    }
}
