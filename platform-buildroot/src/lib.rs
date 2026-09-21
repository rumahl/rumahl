#![cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("rumahl-platform-buildroot requires Linux (or macOS for host tests)");

mod runtime_provider;
mod runtime_secret_server;
mod runtime_secrets;
mod tpm2_key_provider;

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
pub use tpm2_key_provider::{
    Tpm2Authorization, Tpm2SealedKey, Tpm2SealedKeyError, Tpm2UnsealKeyProvider,
    Tpm2UnsealKeyProviderError,
};
