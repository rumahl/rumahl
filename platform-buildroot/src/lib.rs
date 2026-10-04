#![cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("rumahl-platform-buildroot requires Linux (or macOS for host tests)");

mod docker_image_channel;
mod docker_image_importer;
mod docker_runtime_target;
mod namespace_secret_target;
mod package_importer;
mod runtime_provider;
mod runtime_secret_server;
mod runtime_secrets;
mod staged_docker_image_resolver;
mod staged_docker_image_writer;
#[cfg(test)]
mod test_support;
mod tpm2_key_provider;

pub use docker_image_channel::{
    UnixDockerImageImporter, UnixDockerImageImporterConfig, UnixDockerImageImporterConfigError,
    UnixDockerImageImporterError, UnixDockerImageServer, UnixDockerImageServerConfig,
    UnixDockerImageServerConfigError, UnixDockerImageServerError,
};
pub use docker_image_importer::{
    DockerImageImportError, DockerImageImporter, DockerImageImporterConfig,
    DockerImageImporterConfigError, DockerImageTarget, MAX_ARCHIVE_BYTES,
};
pub use docker_runtime_target::{
    DockerImageReference, DockerImageReferenceError, DockerImageResolver, DockerRuntimeTarget,
    DockerRuntimeTargetConfig, DockerRuntimeTargetConfigError, DockerRuntimeTargetError,
};
pub use namespace_secret_target::{
    NamespaceRuntimeSecretTarget, NamespaceRuntimeSecretTargetConfig,
    NamespaceRuntimeSecretTargetConfigError, NamespaceRuntimeSecretTargetError,
};
pub use package_importer::{
    ContainerImageImporter, PackageImportError, PackageImporter, PackageImporterConfig,
    PackageImporterConfigError, PublishedInstall,
};
pub use runtime_provider::{
    RuntimeControlTarget, RuntimeControlTargetOutcome, RuntimeInstallationSpec,
    UnixAppRuntimeProvider, UnixAppRuntimeProviderConfig, UnixAppRuntimeProviderConfigError,
    UnixAppRuntimeProviderError, UnixRuntimeControlServer, UnixRuntimeControlServerConfig,
    UnixRuntimeControlServerConfigError, UnixRuntimeControlServerError,
};
pub use runtime_secret_server::{
    RuntimeOidcClientSecret, RuntimeSecretRemoval, RuntimeSecretTarget, RuntimeSecretTargetOutcome,
    UnixRuntimeSecretServer, UnixRuntimeSecretServerConfig, UnixRuntimeSecretServerConfigError,
    UnixRuntimeSecretServerError,
};
pub use runtime_secrets::{
    UnixRuntimeSecretDelivery, UnixRuntimeSecretDeliveryConfig,
    UnixRuntimeSecretDeliveryConfigError, UnixRuntimeSecretDeliveryError,
};
pub use staged_docker_image_resolver::{
    StagedDockerImageResolver, StagedDockerImageResolverConfig,
    StagedDockerImageResolverConfigError, StagedDockerImageResolverError,
};
pub use staged_docker_image_writer::{
    StagedDockerImageWriter, StagedDockerImageWriterConfig, StagedDockerImageWriterConfigError,
    StagedDockerImageWriterError,
};
pub use tpm2_key_provider::{
    Tpm2Authorization, Tpm2SealedKey, Tpm2SealedKeyError, Tpm2UnsealKeyProvider,
    Tpm2UnsealKeyProviderError,
};
