use crate::{
    AppIdentity, AppManifest, AppManifestValidator, InstallationId, InstalledApp, InstalledAppError,
};

#[derive(Debug, Clone)]
pub struct InstalledAppSnapshot {
    app: InstalledApp,
}

impl InstalledAppSnapshot {
    pub fn new(identity: AppIdentity, manifest: AppManifest) -> Result<Self, InstalledAppError> {
        let app = InstalledApp::restore(identity, manifest, &AppManifestValidator::new())?;

        Ok(Self { app })
    }

    pub fn capture(app: &InstalledApp) -> Self {
        Self { app: app.clone() }
    }

    pub fn identity(&self) -> &AppIdentity {
        self.app.identity()
    }

    pub fn installation_id(&self) -> &InstallationId {
        self.app.installation_id()
    }

    pub fn manifest(&self) -> &AppManifest {
        self.app.manifest()
    }

    pub(crate) fn installed_app(&self) -> &InstalledApp {
        &self.app
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppVersion, PackagePath, PublisherId, RuntimeDescriptor, RuntimeEntrypoint,
        RuntimeEntrypointId,
    };

    fn manifest() -> AppManifest {
        let mut runtime = RuntimeDescriptor::web();

        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap()
    }

    #[test]
    fn captures_installed_app_without_changing_identity() {
        let app = InstalledApp::create(manifest(), &AppManifestValidator::new()).unwrap();

        let snapshot = InstalledAppSnapshot::capture(&app);

        assert_eq!(snapshot.identity(), app.identity());
        assert_eq!(snapshot.manifest(), app.manifest());
    }

    #[test]
    fn rebuilds_validated_snapshot_from_persisted_parts() {
        let manifest = manifest();

        let identity = AppIdentity::new(
            manifest.app_id().clone(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

        let installation_id = *identity.installation_id();

        let snapshot = InstalledAppSnapshot::new(identity, manifest).unwrap();

        assert_eq!(snapshot.installation_id(), &installation_id);
    }

    #[test]
    fn rejects_persisted_identity_that_does_not_match_manifest() {
        let manifest = manifest();

        let identity = AppIdentity::new(
            AppId::parse("com.rumahl.files").unwrap(),
            InstallationId::new(),
            manifest.publisher_id().clone(),
        );

        assert_eq!(
            InstalledAppSnapshot::new(identity, manifest).unwrap_err(),
            InstalledAppError::AppIdMismatch
        );
    }
}
