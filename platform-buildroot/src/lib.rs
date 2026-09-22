#![cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("rumahl-platform-buildroot requires Linux (or macOS for host tests)");

mod docker_runtime_target;
mod namespace_secret_target;
mod runtime_provider;
mod runtime_secret_server;
mod runtime_secrets;
mod staged_docker_image_resolver;
#[cfg(test)]
mod test_support;
mod tpm2_key_provider;

pub use docker_runtime_target::{
    DockerImageReference, DockerImageReferenceError, DockerImageResolver, DockerRuntimeTarget,
    DockerRuntimeTargetConfig, DockerRuntimeTargetConfigError, DockerRuntimeTargetError,
};
pub use namespace_secret_target::{
    NamespaceRuntimeSecretTarget, NamespaceRuntimeSecretTargetConfig,
    NamespaceRuntimeSecretTargetConfigError, NamespaceRuntimeSecretTargetError,
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
pub use tpm2_key_provider::{
    Tpm2Authorization, Tpm2SealedKey, Tpm2SealedKeyError, Tpm2UnsealKeyProvider,
    Tpm2UnsealKeyProviderError,
};
