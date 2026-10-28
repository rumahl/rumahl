//! Durable orchestration for cross-resource app lifecycle operations.
//!
//! The runner deliberately depends only on provider contracts. Concrete
//! SQLite, runtime, and platform adapters remain outside this crate.

mod runner;
mod runtime_secrets;

pub use runner::{
    AppInstallResult, AppOperationRecoveryReport, AppOperationRunner, AppOperationRunnerError,
    AppRuntimeServices, AppUninstallResult,
};
pub use runtime_secrets::RuntimeSecretDelivery;
