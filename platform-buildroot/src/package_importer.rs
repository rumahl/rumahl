use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rumahl_app_operations::{AppOperationRunner, AppOperationRunnerError, RuntimeSecretDelivery};
use rumahl_app_packages::{
    PackageVerificationError, PackageVerifier, PublishedAssets, TrustStore, TrustStoreError,
    VerifiedPackage, WebAssetPublishError, WebAssetPublisher, WebAssetPublisherConfig,
    WebAssetPublisherConfigError,
};
use rumahl_core::{
    AppDatabaseProvider, AppId, AppOperation, AppOperationKind, AppOperationRepository,
    AppRuntimeProvider, InMemoryGrantStore, InstallationId, PackagePath,
    PlatformSnapshotRepository, PlatformState, RuntimeEntrypointTarget, RuntimeKind, SecretStore,
};
use rumahl_oidc_provider::{InstalledAppOriginResolver, OidcClientRepository};

use crate::{ContainerImageImporter, DockerImageReference, StagedImage, StagedImageEnvironment};

#[derive(Debug, Clone)]
pub struct PackageImporterConfig {
    trust_store: PathBuf,
    assets: WebAssetPublisherConfig,
}

#[derive(Debug)]
pub enum PackageImporterConfigError {
    TrustStoreRead(io::Error),
    TrustStore(TrustStoreError),
    AssetRoot(WebAssetPublisherConfigError),
}

/// Coordinates verification with publication for one package.
pub struct PackageImporter<I> {
    verifier: PackageVerifier,
    assets: WebAssetPublisher,
    image_importer: I,
}

/// What a publish call produced, depending on the declared runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedInstall {
    Web(PublishedAssets),
    Container {
        /// Number of declared container artifacts.
        images: usize,
        /// The immutable image reference the runtime resolves for this app.
        image: DockerImageReference,
        /// The image's own configuration environment, captured at import.
        environment: Vec<StagedImageEnvironment>,
    },
}

/// A package that was verified, published and (if needed) recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    installation_id: InstallationId,
    published: PublishedInstall,
}

#[derive(Debug)]
pub enum PackageImportError {
    Verification(PackageVerificationError),
    Manifest(rumahl_app_packages::PackageManifestError),
    Publisher(WebAssetPublishError),
    ImageImport(Box<dyn Error + Send + Sync + 'static>),
    Journal(Box<dyn Error + Send + Sync + 'static>),
    Install(Box<AppOperationRunnerError>),
    UnsupportedRuntime(RuntimeKind),
    MissingWebEntrypoint,
    MissingContainerArtifact,
    /// The publisher allowlist policy does not include this package's
    /// publisher.
    PublisherNotAllowed,
}

impl PackageImporterConfig {
    pub fn new(
        trust_store: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
    ) -> Result<Self, PackageImporterConfigError> {
        let assets = WebAssetPublisherConfig::new(asset_root)
            .map_err(PackageImporterConfigError::AssetRoot)?;
        Ok(Self {
            trust_store: trust_store.into(),
            assets,
        })
    }
}

impl<I> PackageImporter<I> {
    pub fn new(
        config: PackageImporterConfig,
        image_importer: I,
    ) -> Result<Self, PackageImporterConfigError> {
        let bytes =
            fs::read(&config.trust_store).map_err(PackageImporterConfigError::TrustStoreRead)?;
        let trust_store =
            TrustStore::from_bytes(&bytes).map_err(PackageImporterConfigError::TrustStore)?;

        Ok(Self {
            verifier: PackageVerifier::new(trust_store),
            assets: WebAssetPublisher::new(config.assets),
            image_importer,
        })
    }

    pub fn verify(&self, package_root: &Path) -> Result<VerifiedPackage, PackageImportError> {
        self.verifier
            .verify(package_root)
            .map_err(PackageImportError::Verification)
    }
}

impl<I: ContainerImageImporter> PackageImporter<I> {
    /// Publishes a verified package for the given installation.
    ///
    /// Web runtimes publish their asset tree; container runtimes import the
    /// declared artifact and record the immutable image reference. Native
    /// runtimes have no import path yet and fail closed.
    pub fn publish(
        &self,
        installation_id: &InstallationId,
        package_root: &Path,
        package: &VerifiedPackage,
    ) -> Result<PublishedInstall, PackageImportError> {
        match package.manifest().app().runtime().kind() {
            RuntimeKind::Web => {
                require_web_entrypoints(package)?;
                let published = self
                    .assets
                    .publish(installation_id, package_root, package)
                    .map_err(PackageImportError::Publisher)?;
                Ok(PublishedInstall::Web(published))
            }
            RuntimeKind::Container => {
                let artifacts = container_artifacts(package)?;
                let mut staged: Option<StagedImage> = None;
                for artifact in &artifacts {
                    let image = self
                        .image_importer
                        .stage_image(installation_id, package_root, artifact)
                        .map_err(|error| PackageImportError::ImageImport(Box::new(error)))?;
                    // A container app runs exactly one image; the runtime
                    // spec refuses more. Report the staged image the runtime
                    // would resolve to, keeping the artifact count for
                    // diagnostics.
                    staged = Some(image);
                }
                let staged = staged.expect("container artifacts are non-empty");
                Ok(PublishedInstall::Container {
                    images: artifacts.len(),
                    image: staged.reference().clone(),
                    environment: staged.environment().to_vec(),
                })
            }
            kind => Err(PackageImportError::UnsupportedRuntime(kind)),
        }
    }

    /// Verifies a package, installs or updates it through the runner and
    /// publishes the package payload.
    ///
    /// An app id that is already installed is updated in place (newer version
    /// required), keeping its installation identity, grants and data. A
    /// container installation or update normally fails its first runtime step
    /// because the new image reference does not exist yet. When the runner
    /// reports an error, the matching incomplete operation is located, the
    /// package is published for its installation id, and startup recovery
    /// resumes the operation forward. A web package can commit directly and is
    /// published afterwards.
    pub fn install<J, D, U, R, O, P, S, T>(
        &self,
        runner: &AppOperationRunner<J, D, U, R, O, P, S, T>,
        package_root: &Path,
        state: &mut PlatformState,
        grants: &mut InMemoryGrantStore,
        allowed_publishers: &[String],
    ) -> Result<InstalledPackage, PackageImportError>
    where
        J: AppOperationRepository,
        D: AppDatabaseProvider,
        U: AppRuntimeProvider,
        R: OidcClientRepository,
        O: InstalledAppOriginResolver,
        P: PlatformSnapshotRepository,
        S: SecretStore,
        T: RuntimeSecretDelivery,
    {
        let verified = self.verify(package_root)?;
        let manifest = verified
            .to_app_manifest()
            .map_err(PackageImportError::Manifest)?;
        // The publisher allowlist is an install policy, not a trust decision:
        // the signature must verify against the trust store either way, and an
        // empty allowlist means every trusted publisher.
        if !allowed_publishers.is_empty()
            && !allowed_publishers
                .iter()
                .any(|publisher| publisher == manifest.publisher_id().as_str())
        {
            return Err(PackageImportError::PublisherNotAllowed);
        }
        let app_id = verified.manifest().app().app_id().clone();
        let update_from = state
            .installed_apps()
            .get_by_app_id(&app_id)
            .map(|app| *app.installation_id());

        // An existing installation is updated in place; the lifecycle rejects
        // versions that do not move forward.
        let started = match update_from {
            Some(installation_id) => runner
                .update(&installation_id, manifest, state, grants)
                .map(|result| (*result.app().installation_id(), AppOperationKind::Update)),
            None => runner
                .install(manifest, state, grants)
                .map(|result| (*result.app().installation_id(), AppOperationKind::Install)),
        };

        match started {
            Ok((installation_id, _kind)) => {
                let published = self.publish(&installation_id, package_root, &verified)?;
                Ok(InstalledPackage {
                    installation_id,
                    published,
                })
            }
            Err(error) => {
                let operations = runner
                    .journal()
                    .list_incomplete()
                    .map_err(|journal| PackageImportError::Journal(Box::new(journal)))?;
                let Some(operation) = select_incomplete_operation(operations, &app_id) else {
                    return Err(PackageImportError::Install(Box::new(error)));
                };
                let installation_id = *operation.installation_id();
                let published = self.publish(&installation_id, package_root, &verified)?;
                runner
                    .recover_incomplete(state, grants)
                    .map_err(|error| PackageImportError::Install(Box::new(error)))?;
                Ok(InstalledPackage {
                    installation_id,
                    published,
                })
            }
        }
    }
}

fn select_incomplete_operation(
    operations: Vec<AppOperation>,
    app_id: &AppId,
) -> Option<AppOperation> {
    operations.into_iter().find(|operation| {
        matches!(
            operation.kind(),
            AppOperationKind::Install | AppOperationKind::Update
        ) && operation
            .target_app()
            .map(|target| target.identity().app_id() == app_id)
            .unwrap_or(false)
    })
}

impl InstalledPackage {
    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn published(&self) -> &PublishedInstall {
        &self.published
    }

    pub fn into_parts(self) -> (InstallationId, PublishedInstall) {
        (self.installation_id, self.published)
    }
}

fn require_web_entrypoints(package: &VerifiedPackage) -> Result<(), PackageImportError> {
    let entrypoints = package
        .manifest()
        .app()
        .runtime()
        .entrypoints()
        .iter()
        .filter_map(|entrypoint| entrypoint.target().package_path());

    let mut found = false;
    for path in entrypoints {
        found = true;
        let listed = package.files().iter().any(|file| file.path() == path);
        if !listed {
            return Err(PackageImportError::Verification(
                PackageVerificationError::MissingFile(path.to_string()),
            ));
        }
    }

    if found {
        Ok(())
    } else {
        Err(PackageImportError::MissingWebEntrypoint)
    }
}

#[cfg(test)]
fn published_container_environment(published: &PublishedInstall) -> &[StagedImageEnvironment] {
    match published {
        PublishedInstall::Container { environment, .. } => environment,
        PublishedInstall::Web(_) => &[],
    }
}

fn container_artifacts(package: &VerifiedPackage) -> Result<Vec<&PackagePath>, PackageImportError> {
    let artifacts = package
        .manifest()
        .app()
        .runtime()
        .entrypoints()
        .iter()
        .filter_map(|entrypoint| match entrypoint.target() {
            RuntimeEntrypointTarget::ContainerArtifact(path) => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();

    if artifacts.is_empty() {
        Err(PackageImportError::MissingContainerArtifact)
    } else {
        Ok(artifacts)
    }
}

impl fmt::Display for PackageImporterConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrustStoreRead(error) => {
                write!(f, "package trust store could not be read: {error}")
            }
            Self::TrustStore(error) => write!(f, "package trust store is invalid: {error}"),
            Self::AssetRoot(error) => write!(f, "web asset root is invalid: {error}"),
        }
    }
}

impl Error for PackageImporterConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TrustStoreRead(error) => Some(error),
            Self::TrustStore(error) => Some(error),
            Self::AssetRoot(error) => Some(error),
        }
    }
}

impl fmt::Display for PackageImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(error) => write!(f, "package verification failed: {error}"),
            Self::Manifest(error) => write!(f, "package manifest is invalid: {error}"),
            Self::Publisher(error) => write!(f, "package publication failed: {error}"),
            Self::ImageImport(error) => write!(f, "image import failed: {error}"),
            Self::Journal(_) => write!(f, "app operation journal failed"),
            Self::Install(error) => write!(f, "app installation failed: {error}"),
            Self::UnsupportedRuntime(kind) => {
                write!(f, "runtime kind {kind:?} cannot be imported")
            }
            Self::MissingWebEntrypoint => {
                write!(f, "web package declares no web-asset entrypoint")
            }
            Self::MissingContainerArtifact => {
                write!(
                    f,
                    "container package declares no container-artifact entrypoint"
                )
            }
            Self::PublisherNotAllowed => {
                write!(
                    f,
                    "the publisher allowlist does not include this package's publisher"
                )
            }
        }
    }
}

impl Error for PackageImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Publisher(error) => Some(error),
            Self::ImageImport(error) => Some(error.as_ref()),
            Self::Journal(error) => Some(error.as_ref()),
            Self::Install(error) => Some(error),
            Self::UnsupportedRuntime(_)
            | Self::MissingWebEntrypoint
            | Self::MissingContainerArtifact
            | Self::PublisherNotAllowed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};
    use rumahl_app_operations::AppRuntimeServices;
    use rumahl_core::{
        AppDatabaseBinding, AppDatabaseInstallationState, AppIdentity, AppOperationId,
        AppRuntimeInstallationState, AppVersion, InstalledApp, PlatformSnapshot, PublisherId,
        RuntimeDescriptor, RuntimeEntrypointId, SecretPurpose, SecretRecord, UnixTimestamp,
    };
    use rumahl_oidc_provider::{OidcClientId, OidcClientRecord, OidcClientSecret};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::*;
    use crate::{
        DockerImageReference, DockerImageResolver, RuntimeInstallationSpec,
        StagedDockerImageResolver, StagedDockerImageResolverConfig, StagedDockerImageWriter,
        test_support::unique_test_root,
    };

    const KEY_ID: &str = "publisher-key-1";
    const PUBLISHER: &str = "com.rumahl";

    struct TestImageImporter {
        image_root: PathBuf,
    }

    impl ContainerImageImporter for TestImageImporter {
        type Error = std::convert::Infallible;

        fn stage_image(
            &self,
            installation_id: &InstallationId,
            _package_root: &Path,
            artifact: &PackagePath,
        ) -> Result<StagedImage, Self::Error> {
            let reference =
                DockerImageReference::parse(format!("sha256:{}", "a".repeat(64))).unwrap();
            StagedDockerImageWriter::new(&self.image_root)
                .unwrap()
                .write(installation_id, artifact, &reference)
                .unwrap();
            Ok(StagedImage::new(
                reference,
                vec![StagedImageEnvironment::parse("PATH=/usr/bin:/bin").unwrap()],
            ))
        }
    }

    struct FailingImageImporter;

    impl ContainerImageImporter for FailingImageImporter {
        type Error = std::io::Error;

        fn stage_image(
            &self,
            _installation_id: &InstallationId,
            _package_root: &Path,
            _artifact: &PackagePath,
        ) -> Result<StagedImage, Self::Error> {
            Err(std::io::Error::other("image import disabled"))
        }
    }

    struct SignedPackage {
        directory: TempDir,
        signing_key: SigningKey,
    }

    fn build_package(
        runtime_kind: &str,
        entrypoints: Value,
        files: &[(&str, &[u8])],
    ) -> SignedPackage {
        build_package_version("1.0.0", runtime_kind, entrypoints, files)
    }

    fn build_package_version(
        version: &str,
        runtime_kind: &str,
        entrypoints: Value,
        files: &[(&str, &[u8])],
    ) -> SignedPackage {
        let directory = tempfile::tempdir().unwrap();
        for (relative, content) in files {
            let path = directory.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);

        let entries = files
            .iter()
            .map(|(relative, content)| {
                json!({
                    "path": relative,
                    "size": content.len(),
                    "sha256": hex(&Sha256::digest(content)),
                })
            })
            .collect::<Vec<Value>>();

        let manifest = json!({
            "formatVersion": 1,
            "publisherId": PUBLISHER,
            "app": {
                "appId": "com.rumahl.notes",
                "version": version,
                "displayName": "Notes",
                "runtime": { "kind": runtime_kind, "entrypoints": entrypoints },
            },
            "files": entries,
        });
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(directory.path().join("package.json"), &manifest_bytes).unwrap();

        let signature = signing_key.sign(&manifest_bytes);
        let envelope = json!({
            "formatVersion": 1,
            "algorithm": "ed25519",
            "keyId": KEY_ID,
            "signature": URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        });
        fs::write(
            directory.path().join("package.sig.json"),
            serde_json::to_vec(&envelope).unwrap(),
        )
        .unwrap();

        SignedPackage {
            directory,
            signing_key,
        }
    }

    fn trust_store_file(package: &SignedPackage) -> (TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("trust-store.json");
        let value = json!({
            "formatVersion": 1,
            "keys": [{
                "keyId": KEY_ID,
                "publisherId": PUBLISHER,
                "publicKey": URL_SAFE_NO_PAD
                    .encode(package.signing_key.verifying_key().to_bytes()),
            }],
        });
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        (directory, path)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn publishes_web_package_assets() {
        let package = build_package(
            "web",
            json!([{ "id": "main", "kind": "web-asset", "path": "frontend/index.html" }]),
            &[("frontend/index.html", b"<html>notes</html>")],
        );
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();

        let verified = importer.verify(package.directory.path()).unwrap();
        let installation_id = InstallationId::new();
        let published = importer
            .publish(&installation_id, package.directory.path(), &verified)
            .unwrap();

        let PublishedInstall::Web(assets) = published else {
            panic!("expected web publication");
        };
        assert_eq!(assets.file_count(), 1);
        assert_eq!(
            fs::read(
                asset_root
                    .join(installation_id.to_string())
                    .join("frontend/index.html")
            )
            .unwrap(),
            b"<html>notes</html>"
        );

        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[test]
    fn stages_container_image_reference() {
        let package = build_package(
            "container",
            json!([
                { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" },
                { "id": "main", "kind": "endpoint", "endpoint": "web" },
            ]),
            &[("runtime/server.oci", b"not-a-real-image")],
        );
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();

        let verified = importer.verify(package.directory.path()).unwrap();
        let installation_id = InstallationId::new();
        let published = importer
            .publish(&installation_id, package.directory.path(), &verified)
            .unwrap();

        assert!(matches!(
            published,
            PublishedInstall::Container { images: 1, .. }
        ));
        assert_eq!(
            published_container_environment(&published),
            &[StagedImageEnvironment::parse("PATH=/usr/bin:/bin").unwrap()]
        );

        let resolver = StagedDockerImageResolver::new(
            StagedDockerImageResolverConfig::new(&image_root).unwrap(),
        );
        let spec = RuntimeInstallationSpec::new(
            AppIdentity::new(
                rumahl_core::AppId::parse("com.rumahl.notes").unwrap(),
                installation_id,
                PublisherId::parse(PUBLISHER).unwrap(),
            ),
            AppVersion::new(1, 0, 0),
            RuntimeDescriptor::container(),
        );
        let image = resolver
            .resolve_image(&spec, &PackagePath::parse("runtime/server.oci").unwrap())
            .unwrap();
        assert_eq!(image.as_str(), format!("sha256:{}", "a".repeat(64)));

        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[test]
    fn propagates_image_import_failure() {
        let package = build_package(
            "container",
            json!([
                { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" },
                { "id": "main", "kind": "endpoint", "endpoint": "web" },
            ]),
            &[("runtime/server.oci", b"not-a-real-image")],
        );
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            FailingImageImporter,
        )
        .unwrap();

        let verified = importer.verify(package.directory.path()).unwrap();
        let error = importer
            .publish(&InstallationId::new(), package.directory.path(), &verified)
            .unwrap_err();

        assert!(matches!(error, PackageImportError::ImageImport(_)));
        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[test]
    fn rejects_unlisted_web_entrypoint() {
        let package = build_package(
            "web",
            json!([{ "id": "main", "kind": "web-asset", "path": "frontend/missing.html" }]),
            &[("frontend/index.html", b"<html>notes</html>")],
        );
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();

        let verified = importer.verify(package.directory.path()).unwrap();
        let error = importer
            .publish(&InstallationId::new(), package.directory.path(), &verified)
            .unwrap_err();

        assert!(matches!(
            error,
            PackageImportError::Verification(PackageVerificationError::MissingFile(_))
        ));
        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[derive(Debug)]
    struct TestError(&'static str);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    #[derive(Default)]
    struct MemoryJournal {
        operations: RefCell<Vec<AppOperation>>,
    }

    impl AppOperationRepository for MemoryJournal {
        type Error = TestError;

        fn create(&self, operation: &AppOperation) -> Result<(), Self::Error> {
            if self
                .operations
                .borrow()
                .iter()
                .any(|stored| stored.id() == operation.id())
            {
                return Err(TestError("duplicate operation"));
            }
            self.operations.borrow_mut().push(operation.clone());
            Ok(())
        }

        fn store_transition(&self, operation: &AppOperation) -> Result<(), Self::Error> {
            let mut operations = self.operations.borrow_mut();
            let stored = operations
                .iter_mut()
                .find(|stored| stored.id() == operation.id())
                .ok_or(TestError("missing operation"))?;
            if stored.revision().checked_add(1) != Some(operation.revision()) {
                return Err(TestError("stale transition"));
            }
            *stored = operation.clone();
            Ok(())
        }

        fn find(&self, id: &AppOperationId) -> Result<Option<AppOperation>, Self::Error> {
            Ok(self
                .operations
                .borrow()
                .iter()
                .find(|operation| operation.id() == id)
                .cloned())
        }

        fn list_incomplete(&self) -> Result<Vec<AppOperation>, Self::Error> {
            Ok(self
                .operations
                .borrow()
                .iter()
                .filter(|operation| !operation.is_terminal())
                .cloned()
                .collect())
        }
    }

    struct NoopDatabases;

    impl AppDatabaseProvider for NoopDatabases {
        type Access = ();
        type Error = TestError;

        fn provision_installation(
            &self,
            _bindings: &[AppDatabaseBinding],
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn installation_state(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<AppDatabaseInstallationState, Self::Error> {
            Ok(AppDatabaseInstallationState::Absent)
        }

        fn access(&self, _binding: &AppDatabaseBinding) -> Result<(), Self::Error> {
            Ok(())
        }

        fn retain_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }

        fn restore_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }
    }

    /// Fails runtime preparation until the image reference has been staged.
    struct StagingRuntime {
        image_root: PathBuf,
    }

    impl StagingRuntime {
        fn staged(&self, installation_id: &InstallationId) -> bool {
            self.image_root
                .join(installation_id.to_string())
                .join("image-reference")
                .is_file()
        }
    }

    impl AppRuntimeProvider for StagingRuntime {
        type Error = TestError;

        fn prepare_installation(&self, app: &InstalledApp) -> Result<(), Self::Error> {
            if self.staged(app.installation_id()) {
                Ok(())
            } else {
                Err(TestError("image is not staged"))
            }
        }

        fn activate_installation(&self, _app: &InstalledApp) -> Result<(), Self::Error> {
            Ok(())
        }

        fn installation_state(
            &self,
            installation_id: &InstallationId,
        ) -> Result<AppRuntimeInstallationState, Self::Error> {
            Ok(if self.staged(installation_id) {
                AppRuntimeInstallationState::Active
            } else {
                AppRuntimeInstallationState::Absent
            })
        }

        fn deactivate_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }

        fn remove_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }
    }

    struct NoopOidc;

    impl OidcClientRepository for NoopOidc {
        type Error = TestError;

        fn insert(&self, _client: &OidcClientRecord) -> Result<(), Self::Error> {
            Ok(())
        }

        fn find_active_by_id(
            &self,
            _client_id: &OidcClientId,
        ) -> Result<Option<OidcClientRecord>, Self::Error> {
            Ok(None)
        }

        fn find_active_by_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<Option<OidcClientRecord>, Self::Error> {
            Ok(None)
        }

        fn revoke_for_installation(
            &self,
            _installation_id: &InstallationId,
            _revoked_at: UnixTimestamp,
        ) -> Result<usize, Self::Error> {
            Ok(0)
        }
    }

    struct TestOrigin;

    impl InstalledAppOriginResolver for TestOrigin {
        type Error = TestError;

        fn resolve_origin(
            &self,
            _app: &InstalledApp,
            _entrypoint: &RuntimeEntrypointId,
        ) -> Result<String, Self::Error> {
            Ok("https://apps.rumahl.test".to_owned())
        }
    }

    #[derive(Default)]
    struct MemorySnapshot {
        snapshot: RefCell<Option<PlatformSnapshot>>,
    }

    impl PlatformSnapshotRepository for MemorySnapshot {
        type Error = TestError;

        fn load(&self) -> Result<Option<PlatformSnapshot>, Self::Error> {
            Ok(self.snapshot.borrow().clone())
        }

        fn store(&self, snapshot: &PlatformSnapshot) -> Result<(), Self::Error> {
            self.snapshot.replace(Some(snapshot.clone()));
            Ok(())
        }
    }

    struct NoopSecrets;

    impl SecretStore for NoopSecrets {
        type Error = TestError;

        fn insert(&self, _secret: &SecretRecord) -> Result<(), Self::Error> {
            Ok(())
        }

        fn find_by_owner_and_purpose(
            &self,
            _owner: &AppIdentity,
            _purpose: &SecretPurpose,
        ) -> Result<Option<SecretRecord>, Self::Error> {
            Ok(None)
        }

        fn remove_for_installation(
            &self,
            _installation_id: &InstallationId,
        ) -> Result<usize, Self::Error> {
            Ok(0)
        }
    }

    struct NoopSecretDelivery;

    impl RuntimeSecretDelivery for NoopSecretDelivery {
        type Error = TestError;

        fn deliver_oidc_client_secret(
            &self,
            _operation_id: &AppOperationId,
            _app: &InstalledApp,
            _client_id: &OidcClientId,
            _client_secret: &OidcClientSecret,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn remove_for_installation(
            &self,
            _operation_id: &AppOperationId,
            _installation_id: &InstallationId,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn container_package() -> SignedPackage {
        build_package(
            "container",
            json!([
                { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" },
                { "id": "main", "kind": "endpoint", "endpoint": "web" },
            ]),
            &[("runtime/server.oci", b"not-a-real-image")],
        )
    }

    fn container_package_version(version: &str) -> SignedPackage {
        build_package_version(
            version,
            "container",
            json!([
                { "id": "service", "kind": "container-artifact", "path": "runtime/server.oci" },
                { "id": "main", "kind": "endpoint", "endpoint": "web" },
            ]),
            &[("runtime/server.oci", b"not-a-real-image")],
        )
    }

    #[test]
    fn updates_an_installed_package_in_place() {
        let v1 = container_package();
        let (_trust_dir, trust_store) = trust_store_file(&v1);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();
        let runner = AppOperationRunner::new(
            MemoryJournal::default(),
            NoopDatabases,
            AppRuntimeServices::new(
                StagingRuntime {
                    image_root: image_root.clone(),
                },
                NoopSecretDelivery,
            ),
            NoopOidc,
            TestOrigin,
            MemorySnapshot::default(),
            NoopSecrets,
        );
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        let first = importer
            .install(&runner, v1.directory.path(), &mut state, &mut grants, &[])
            .unwrap();
        let installation_id = *first.installation_id();
        let identity = state.installed_apps().apps()[0].identity().clone();
        let user = rumahl_core::UserId::new();
        let mut policy = rumahl_core::GrantIssuerPolicy::new();
        policy.set_user_role(user, rumahl_core::UserRole::User);
        grants.insert(
            rumahl_core::GrantAuthority::new()
                .issue(
                    &policy,
                    rumahl_core::UserIdentity::new(user).into(),
                    identity.clone().into(),
                    rumahl_core::PermissionId::parse("rumahl.files.read").unwrap(),
                    rumahl_core::PermissionScope::Explicit,
                    vec![rumahl_core::ResourceRef::new(
                        rumahl_core::ResourceNamespace::parse("rumahl.files").unwrap(),
                        rumahl_core::ResourceKind::parse("file").unwrap(),
                        rumahl_core::ResourceKey::parse("document-1").unwrap(),
                    )],
                )
                .unwrap(),
        );

        // The next version of the same app installs as an update, not a second
        // installation.
        let v2 = container_package_version("1.1.0");
        let updated = importer
            .install(&runner, v2.directory.path(), &mut state, &mut grants, &[])
            .unwrap();

        assert_eq!(updated.installation_id(), &installation_id);
        assert_eq!(state.installed_apps().len(), 1);
        assert_eq!(
            state.installed_apps().apps()[0].manifest().version(),
            &AppVersion::new(1, 1, 0)
        );
        assert_eq!(state.installed_apps().apps()[0].identity(), &identity);
        // Grants survive the update.
        assert_eq!(grants.len(), 1);
        assert!(runner.journal().list_incomplete().unwrap().is_empty());

        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[test]
    fn the_publisher_allowlist_decides_which_packages_install() {
        let package = container_package();
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();
        let runner = AppOperationRunner::new(
            MemoryJournal::default(),
            NoopDatabases,
            AppRuntimeServices::new(
                StagingRuntime {
                    image_root: image_root.clone(),
                },
                NoopSecretDelivery,
            ),
            NoopOidc,
            TestOrigin,
            MemorySnapshot::default(),
            NoopSecrets,
        );
        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        // An allowlist that does not include this publisher refuses the
        // install before anything is staged or journaled.
        let refused = importer.install(
            &runner,
            package.directory.path(),
            &mut state,
            &mut grants,
            &["org.example".to_owned()],
        );
        assert!(matches!(
            refused,
            Err(PackageImportError::PublisherNotAllowed)
        ));
        assert_eq!(state.installed_apps().len(), 0);
        assert!(runner.journal().list_incomplete().unwrap().is_empty());

        // An allowlist that includes the publisher installs normally. (An
        // empty allowlist meaning "every trusted publisher" is what every
        // other install test exercises with `&[]`.)
        importer
            .install(
                &runner,
                package.directory.path(),
                &mut state,
                &mut grants,
                &[PUBLISHER.to_owned()],
            )
            .unwrap();
        assert_eq!(state.installed_apps().len(), 1);

        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }

    #[test]
    fn installs_container_package_after_publishing_image() {
        let package = container_package();
        let (_trust_dir, trust_store) = trust_store_file(&package);
        let asset_root = unique_test_root('a');
        let image_root = unique_test_root('g');
        let importer = PackageImporter::new(
            PackageImporterConfig::new(&trust_store, &asset_root).unwrap(),
            TestImageImporter {
                image_root: image_root.clone(),
            },
        )
        .unwrap();

        let runner = AppOperationRunner::new(
            MemoryJournal::default(),
            NoopDatabases,
            AppRuntimeServices::new(
                StagingRuntime {
                    image_root: image_root.clone(),
                },
                NoopSecretDelivery,
            ),
            NoopOidc,
            TestOrigin,
            MemorySnapshot::default(),
            NoopSecrets,
        );

        let mut state = PlatformState::new();
        let mut grants = InMemoryGrantStore::new();

        let installed = importer
            .install(
                &runner,
                package.directory.path(),
                &mut state,
                &mut grants,
                &[],
            )
            .unwrap();

        assert_eq!(state.installed_apps().len(), 1);
        assert_eq!(
            installed.installation_id(),
            state.installed_apps().apps()[0].installation_id()
        );
        assert!(matches!(
            installed.published(),
            PublishedInstall::Container { images: 1, .. }
        ));
        assert!(runner.journal().list_incomplete().unwrap().is_empty());

        fs::remove_dir_all(asset_root).unwrap();
        fs::remove_dir_all(image_root).unwrap();
    }
}
