use super::{PackagePath, RuntimeEndpointId, RuntimeEntrypointId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEntrypointKind {
    WebAsset,
    ContainerArtifact,
    Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEntrypointTarget {
    WebAsset(PackagePath),
    ContainerArtifact(PackagePath),
    Endpoint(RuntimeEndpointId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEntrypoint {
    id: RuntimeEntrypointId,
    target: RuntimeEntrypointTarget,
}

impl RuntimeEntrypoint {
    pub fn web_asset(id: RuntimeEntrypointId, path: PackagePath) -> Self {
        Self {
            id,
            target: RuntimeEntrypointTarget::WebAsset(path),
        }
    }

    pub fn container_artifact(id: RuntimeEntrypointId, artifact: PackagePath) -> Self {
        Self {
            id,
            target: RuntimeEntrypointTarget::ContainerArtifact(artifact),
        }
    }

    pub fn endpoint(id: RuntimeEntrypointId, endpoint: RuntimeEndpointId) -> Self {
        Self {
            id,
            target: RuntimeEntrypointTarget::Endpoint(endpoint),
        }
    }

    pub fn id(&self) -> &RuntimeEntrypointId {
        &self.id
    }

    pub fn target(&self) -> &RuntimeEntrypointTarget {
        &self.target
    }

    pub fn kind(&self) -> RuntimeEntrypointKind {
        self.target.kind()
    }
}

impl RuntimeEntrypointTarget {
    pub fn kind(&self) -> RuntimeEntrypointKind {
        match self {
            Self::WebAsset(_) => RuntimeEntrypointKind::WebAsset,
            Self::ContainerArtifact(_) => RuntimeEntrypointKind::ContainerArtifact,
            Self::Endpoint(_) => RuntimeEntrypointKind::Endpoint,
        }
    }

    pub fn package_path(&self) -> Option<&PackagePath> {
        match self {
            Self::WebAsset(path) | Self::ContainerArtifact(path) => Some(path),
            Self::Endpoint(_) => None,
        }
    }

    pub fn endpoint_id(&self) -> Option<&RuntimeEndpointId> {
        match self {
            Self::Endpoint(endpoint) => Some(endpoint),
            Self::WebAsset(_) | Self::ContainerArtifact(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_web_asset_entrypoint() {
        let entrypoint = RuntimeEntrypoint::web_asset(
            RuntimeEntrypointId::parse("main").unwrap(),
            PackagePath::parse("frontend/index.html").unwrap(),
        );

        assert_eq!(entrypoint.id().as_str(), "main");
        assert_eq!(entrypoint.kind(), RuntimeEntrypointKind::WebAsset);
        assert_eq!(
            entrypoint.target().package_path().unwrap().as_str(),
            "frontend/index.html"
        );
    }

    #[test]
    fn creates_container_targets() {
        let service = RuntimeEntrypoint::container_artifact(
            RuntimeEntrypointId::parse("service").unwrap(),
            PackagePath::parse("runtime/server.oci").unwrap(),
        );

        let main = RuntimeEntrypoint::endpoint(
            RuntimeEntrypointId::parse("main").unwrap(),
            RuntimeEndpointId::parse("web").unwrap(),
        );

        assert_eq!(service.kind(), RuntimeEntrypointKind::ContainerArtifact);
        assert_eq!(main.kind(), RuntimeEntrypointKind::Endpoint);
        assert_eq!(main.target().endpoint_id().unwrap().as_str(), "web");
    }
}
