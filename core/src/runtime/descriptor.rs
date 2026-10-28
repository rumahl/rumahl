use std::error::Error;
use std::fmt;

use super::{RuntimeEntrypoint, RuntimeEntrypointId, RuntimeEntrypointKind, RuntimeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDescriptor {
    kind: RuntimeKind,
    entrypoints: Vec<RuntimeEntrypoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDescriptorError {
    DuplicateEntrypoint,
    IncompatibleEntrypoint {
        runtime: RuntimeKind,
        entrypoint: RuntimeEntrypointKind,
    },
}

impl RuntimeDescriptor {
    pub fn new(kind: RuntimeKind) -> Self {
        Self {
            kind,
            entrypoints: Vec::new(),
        }
    }

    pub fn web() -> Self {
        Self::new(RuntimeKind::Web)
    }

    pub fn container() -> Self {
        Self::new(RuntimeKind::Container)
    }

    pub fn native() -> Self {
        Self::new(RuntimeKind::Native)
    }

    pub fn kind(&self) -> RuntimeKind {
        self.kind
    }

    pub fn add_entrypoint(
        &mut self,
        entrypoint: RuntimeEntrypoint,
    ) -> Result<(), RuntimeDescriptorError> {
        if !self.accepts(entrypoint.kind()) {
            return Err(RuntimeDescriptorError::IncompatibleEntrypoint {
                runtime: self.kind,
                entrypoint: entrypoint.kind(),
            });
        }

        if self
            .entrypoints
            .iter()
            .any(|existing| existing.id() == entrypoint.id())
        {
            return Err(RuntimeDescriptorError::DuplicateEntrypoint);
        }

        self.entrypoints.push(entrypoint);

        Ok(())
    }

    pub fn entrypoint(&self, id: &RuntimeEntrypointId) -> Option<&RuntimeEntrypoint> {
        self.entrypoints
            .iter()
            .find(|entrypoint| entrypoint.id() == id)
    }

    pub fn entrypoints(&self) -> &[RuntimeEntrypoint] {
        &self.entrypoints
    }

    fn accepts(&self, entrypoint: RuntimeEntrypointKind) -> bool {
        matches!(
            (self.kind, entrypoint),
            (RuntimeKind::Web, RuntimeEntrypointKind::WebAsset)
                | (
                    RuntimeKind::Container,
                    RuntimeEntrypointKind::ContainerArtifact | RuntimeEntrypointKind::Endpoint
                )
        )
    }
}

impl fmt::Display for RuntimeDescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEntrypoint => {
                write!(f, "runtime entrypoint id is already declared")
            }

            Self::IncompatibleEntrypoint {
                runtime,
                entrypoint,
            } => {
                write!(
                    f,
                    "runtime entrypoint kind '{entrypoint:?}' is incompatible with runtime '{runtime:?}'"
                )
            }
        }
    }
}

impl Error for RuntimeDescriptorError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{PackagePath, RuntimeEndpointId};

    #[test]
    fn creates_web_runtime_descriptor() {
        assert_eq!(RuntimeDescriptor::web().kind(), RuntimeKind::Web);
    }

    #[test]
    fn creates_container_runtime_descriptor() {
        assert_eq!(
            RuntimeDescriptor::container().kind(),
            RuntimeKind::Container
        );
    }

    #[test]
    fn creates_native_runtime_descriptor() {
        assert_eq!(RuntimeDescriptor::native().kind(), RuntimeKind::Native);
    }

    #[test]
    fn web_runtime_accepts_package_asset_entrypoint() {
        let mut runtime = RuntimeDescriptor::web();

        let id = RuntimeEntrypointId::parse("main").unwrap();

        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                id.clone(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();

        assert_eq!(runtime.entrypoints().len(), 1);
        assert_eq!(runtime.entrypoint(&id).unwrap().id(), &id);
    }

    #[test]
    fn container_runtime_accepts_artifact_and_endpoint_entrypoints() {
        let mut runtime = RuntimeDescriptor::container();

        runtime
            .add_entrypoint(RuntimeEntrypoint::container_artifact(
                RuntimeEntrypointId::parse("service").unwrap(),
                PackagePath::parse("runtime/server.oci").unwrap(),
            ))
            .unwrap();

        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                RuntimeEntrypointId::parse("main").unwrap(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();

        assert_eq!(runtime.entrypoints().len(), 2);
    }

    #[test]
    fn rejects_entrypoint_incompatible_with_runtime() {
        let mut runtime = RuntimeDescriptor::web();

        let result = runtime.add_entrypoint(RuntimeEntrypoint::container_artifact(
            RuntimeEntrypointId::parse("service").unwrap(),
            PackagePath::parse("runtime/server.oci").unwrap(),
        ));

        assert_eq!(
            result.unwrap_err(),
            RuntimeDescriptorError::IncompatibleEntrypoint {
                runtime: RuntimeKind::Web,
                entrypoint: RuntimeEntrypointKind::ContainerArtifact,
            }
        );
    }

    #[test]
    fn native_runtime_rejects_unmodeled_entrypoint() {
        let mut runtime = RuntimeDescriptor::native();

        let result = runtime.add_entrypoint(RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        ));

        assert_eq!(
            result.unwrap_err(),
            RuntimeDescriptorError::IncompatibleEntrypoint {
                runtime: RuntimeKind::Native,
                entrypoint: RuntimeEntrypointKind::WebAsset,
            }
        );
    }

    #[test]
    fn rejects_duplicate_entrypoint_id() {
        let mut runtime = RuntimeDescriptor::container();

        let id = RuntimeEntrypointId::parse("main").unwrap();

        runtime
            .add_entrypoint(RuntimeEntrypoint::endpoint(
                id.clone(),
                RuntimeEndpointId::parse("web").unwrap(),
            ))
            .unwrap();

        let result = runtime.add_entrypoint(RuntimeEntrypoint::container_artifact(
            id,
            PackagePath::parse("runtime/server.oci").unwrap(),
        ));

        assert_eq!(
            result.unwrap_err(),
            RuntimeDescriptorError::DuplicateEntrypoint
        );
    }
}
