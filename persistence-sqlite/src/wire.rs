use std::error::Error;
use std::fmt;

use rumahl_core::{
    AppId, AppIdentity, AppManifest, AppVersion, CapabilityId, CommandAction,
    CommandContributionDeclaration, ContributionDeclaration, ContributionId, EventName, GrantId,
    Identity, InstallationId, InstalledAppSnapshot, OidcCallbackPath, OidcClientDeclaration,
    OidcClientType, OidcScope, PackagePath, PermissionGrantSnapshot, PermissionId,
    PermissionRequest, PermissionScope, PlatformSnapshot, PublisherId, ResourceKey, ResourceKind,
    ResourceNamespace, ResourceRef, RuntimeDescriptor, RuntimeEndpointId, RuntimeEntrypoint,
    RuntimeEntrypointId, RuntimeEntrypointTarget, RuntimeKind, ServiceId, ServiceIdentity, UserId,
    UserIdentity,
};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub(crate) struct WireSnapshotError {
    message: String,
}

impl WireSnapshotError {
    fn invalid(field: &str, error: impl fmt::Display) -> Self {
        Self {
            message: format!("invalid {field}: {error}"),
        }
    }
}

impl fmt::Display for WireSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for WireSnapshotError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireSnapshot {
    version: u32,
    installed_apps: Vec<WireInstalledApp>,
    grants: Vec<WireGrant>,
}

impl WireSnapshot {
    pub(crate) fn capture(snapshot: &PlatformSnapshot) -> Self {
        Self {
            version: snapshot.version(),
            installed_apps: snapshot
                .installed_apps()
                .iter()
                .map(WireInstalledApp::capture)
                .collect(),
            grants: snapshot.grants().iter().map(WireGrant::capture).collect(),
        }
    }

    pub(crate) fn version(&self) -> u32 {
        self.version
    }

    pub(crate) fn into_domain(self) -> Result<PlatformSnapshot, WireSnapshotError> {
        let installed_apps = self
            .installed_apps
            .into_iter()
            .map(WireInstalledApp::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let grants = self
            .grants
            .into_iter()
            .map(WireGrant::into_domain)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PlatformSnapshot::new(self.version, installed_apps, grants))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireInstalledApp {
    identity: WireAppIdentity,
    manifest: WireManifest,
}

impl WireInstalledApp {
    fn capture(snapshot: &InstalledAppSnapshot) -> Self {
        Self {
            identity: WireAppIdentity::capture(snapshot.identity()),
            manifest: WireManifest::capture(snapshot.manifest()),
        }
    }

    fn into_domain(self) -> Result<InstalledAppSnapshot, WireSnapshotError> {
        InstalledAppSnapshot::new(self.identity.into_domain()?, self.manifest.into_domain()?)
            .map_err(|error| WireSnapshotError::invalid("installed app", error))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAppIdentity {
    app_id: String,
    installation_id: String,
    publisher_id: String,
}

impl WireAppIdentity {
    fn capture(identity: &AppIdentity) -> Self {
        Self {
            app_id: identity.app_id().as_str().to_owned(),
            installation_id: identity.installation_id().to_string(),
            publisher_id: identity.publisher_id().as_str().to_owned(),
        }
    }

    fn into_domain(self) -> Result<AppIdentity, WireSnapshotError> {
        Ok(AppIdentity::new(
            AppId::parse(&self.app_id)
                .map_err(|error| WireSnapshotError::invalid("app identity app_id", error))?,
            InstallationId::parse(&self.installation_id).map_err(|error| {
                WireSnapshotError::invalid("app identity installation_id", error)
            })?,
            PublisherId::parse(&self.publisher_id)
                .map_err(|error| WireSnapshotError::invalid("app identity publisher_id", error))?,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManifest {
    app_id: String,
    publisher_id: String,
    version: String,
    display_name: String,
    runtime: WireRuntime,
    permission_requests: Vec<WirePermissionRequest>,
    provided_capabilities: Vec<String>,
    contributions: Vec<WireContribution>,
    event_subscriptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oidc_client: Option<WireOidcClient>,
}

impl WireManifest {
    fn capture(manifest: &AppManifest) -> Self {
        Self {
            app_id: manifest.app_id().as_str().to_owned(),
            publisher_id: manifest.publisher_id().as_str().to_owned(),
            version: manifest.version().to_string(),
            display_name: manifest.display_name().to_owned(),
            runtime: WireRuntime::capture(manifest.runtime()),
            permission_requests: manifest
                .permission_requests()
                .iter()
                .map(WirePermissionRequest::capture)
                .collect(),
            provided_capabilities: manifest
                .provided_capabilities()
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            contributions: manifest
                .contributions()
                .iter()
                .map(WireContribution::capture)
                .collect(),
            event_subscriptions: manifest
                .event_subscriptions()
                .iter()
                .map(|event| event.as_str().to_owned())
                .collect(),
            oidc_client: manifest.oidc_client().map(WireOidcClient::capture),
        }
    }

    fn into_domain(self) -> Result<AppManifest, WireSnapshotError> {
        let mut manifest = AppManifest::new(
            AppId::parse(&self.app_id)
                .map_err(|error| WireSnapshotError::invalid("manifest app_id", error))?,
            PublisherId::parse(&self.publisher_id)
                .map_err(|error| WireSnapshotError::invalid("manifest publisher_id", error))?,
            AppVersion::parse(&self.version)
                .map_err(|error| WireSnapshotError::invalid("manifest version", error))?,
            self.display_name,
            self.runtime.into_domain()?,
        )
        .map_err(|error| WireSnapshotError::invalid("manifest", error))?;

        for request in self.permission_requests {
            manifest
                .add_permission_request(request.into_domain()?)
                .map_err(|error| {
                    WireSnapshotError::invalid("manifest permission request", error)
                })?;
        }

        for capability in self.provided_capabilities {
            manifest
                .add_provided_capability(CapabilityId::parse(&capability).map_err(|error| {
                    WireSnapshotError::invalid("manifest provided capability", error)
                })?)
                .map_err(|error| {
                    WireSnapshotError::invalid("manifest provided capability", error)
                })?;
        }

        for contribution in self.contributions {
            manifest
                .add_contribution(contribution.into_domain()?)
                .map_err(|error| WireSnapshotError::invalid("manifest contribution", error))?;
        }

        for event in self.event_subscriptions {
            manifest
                .add_event_subscription(EventName::parse(&event).map_err(|error| {
                    WireSnapshotError::invalid("manifest event subscription", error)
                })?)
                .map_err(|error| {
                    WireSnapshotError::invalid("manifest event subscription", error)
                })?;
        }

        if let Some(oidc_client) = self.oidc_client {
            manifest
                .declare_oidc_client(oidc_client.into_domain()?)
                .map_err(|error| WireSnapshotError::invalid("manifest OIDC client", error))?;
        }

        Ok(manifest)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOidcClient {
    client_type: WireOidcClientType,
    callback_entrypoint: String,
    callback_path: String,
    scopes: Vec<WireOidcScope>,
}

impl WireOidcClient {
    fn capture(declaration: &OidcClientDeclaration) -> Self {
        Self {
            client_type: declaration.client_type().into(),
            callback_entrypoint: declaration.callback_entrypoint().as_str().to_owned(),
            callback_path: declaration.callback_path().as_str().to_owned(),
            scopes: declaration
                .scopes()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        }
    }

    fn into_domain(self) -> Result<OidcClientDeclaration, WireSnapshotError> {
        OidcClientDeclaration::new(
            self.client_type.into(),
            RuntimeEntrypointId::parse(self.callback_entrypoint)
                .map_err(|error| WireSnapshotError::invalid("OIDC callback entrypoint", error))?,
            OidcCallbackPath::parse(self.callback_path)
                .map_err(|error| WireSnapshotError::invalid("OIDC callback path", error))?,
            self.scopes.into_iter().map(Into::into).collect(),
        )
        .map_err(|error| WireSnapshotError::invalid("OIDC client declaration", error))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireOidcClientType {
    Public,
    Confidential,
}

impl From<OidcClientType> for WireOidcClientType {
    fn from(value: OidcClientType) -> Self {
        match value {
            OidcClientType::Public => Self::Public,
            OidcClientType::Confidential => Self::Confidential,
        }
    }
}

impl From<WireOidcClientType> for OidcClientType {
    fn from(value: WireOidcClientType) -> Self {
        match value {
            WireOidcClientType::Public => Self::Public,
            WireOidcClientType::Confidential => Self::Confidential,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireOidcScope {
    OpenId,
    Profile,
    Email,
    OfflineAccess,
}

impl From<OidcScope> for WireOidcScope {
    fn from(value: OidcScope) -> Self {
        match value {
            OidcScope::OpenId => Self::OpenId,
            OidcScope::Profile => Self::Profile,
            OidcScope::Email => Self::Email,
            OidcScope::OfflineAccess => Self::OfflineAccess,
        }
    }
}

impl From<WireOidcScope> for OidcScope {
    fn from(value: WireOidcScope) -> Self {
        match value {
            WireOidcScope::OpenId => Self::OpenId,
            WireOidcScope::Profile => Self::Profile,
            WireOidcScope::Email => Self::Email,
            WireOidcScope::OfflineAccess => Self::OfflineAccess,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRuntime {
    kind: WireRuntimeKind,
    entrypoints: Vec<WireRuntimeEntrypoint>,
}

impl WireRuntime {
    fn capture(runtime: &RuntimeDescriptor) -> Self {
        Self {
            kind: runtime.kind().into(),
            entrypoints: runtime
                .entrypoints()
                .iter()
                .map(WireRuntimeEntrypoint::capture)
                .collect(),
        }
    }

    fn into_domain(self) -> Result<RuntimeDescriptor, WireSnapshotError> {
        let mut runtime = RuntimeDescriptor::new(self.kind.into());

        for entrypoint in self.entrypoints {
            runtime
                .add_entrypoint(entrypoint.into_domain()?)
                .map_err(|error| WireSnapshotError::invalid("runtime entrypoint", error))?;
        }

        Ok(runtime)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireRuntimeKind {
    Web,
    Container,
    Native,
}

impl From<RuntimeKind> for WireRuntimeKind {
    fn from(kind: RuntimeKind) -> Self {
        match kind {
            RuntimeKind::Web => Self::Web,
            RuntimeKind::Container => Self::Container,
            RuntimeKind::Native => Self::Native,
        }
    }
}

impl From<WireRuntimeKind> for RuntimeKind {
    fn from(kind: WireRuntimeKind) -> Self {
        match kind {
            WireRuntimeKind::Web => Self::Web,
            WireRuntimeKind::Container => Self::Container,
            WireRuntimeKind::Native => Self::Native,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum WireRuntimeEntrypoint {
    WebAsset { id: String, path: String },
    ContainerArtifact { id: String, path: String },
    Endpoint { id: String, endpoint: String },
}

impl WireRuntimeEntrypoint {
    fn capture(entrypoint: &RuntimeEntrypoint) -> Self {
        let id = entrypoint.id().as_str().to_owned();

        match entrypoint.target() {
            RuntimeEntrypointTarget::WebAsset(path) => Self::WebAsset {
                id,
                path: path.as_str().to_owned(),
            },
            RuntimeEntrypointTarget::ContainerArtifact(path) => Self::ContainerArtifact {
                id,
                path: path.as_str().to_owned(),
            },
            RuntimeEntrypointTarget::Endpoint(endpoint) => Self::Endpoint {
                id,
                endpoint: endpoint.as_str().to_owned(),
            },
        }
    }

    fn into_domain(self) -> Result<RuntimeEntrypoint, WireSnapshotError> {
        match self {
            Self::WebAsset { id, path } => Ok(RuntimeEntrypoint::web_asset(
                parse_entrypoint_id(&id)?,
                PackagePath::parse(&path)
                    .map_err(|error| WireSnapshotError::invalid("web asset path", error))?,
            )),
            Self::ContainerArtifact { id, path } => Ok(RuntimeEntrypoint::container_artifact(
                parse_entrypoint_id(&id)?,
                PackagePath::parse(&path).map_err(|error| {
                    WireSnapshotError::invalid("container artifact path", error)
                })?,
            )),
            Self::Endpoint { id, endpoint } => Ok(RuntimeEntrypoint::endpoint(
                parse_entrypoint_id(&id)?,
                RuntimeEndpointId::parse(&endpoint)
                    .map_err(|error| WireSnapshotError::invalid("runtime endpoint", error))?,
            )),
        }
    }
}

fn parse_entrypoint_id(value: &str) -> Result<RuntimeEntrypointId, WireSnapshotError> {
    RuntimeEntrypointId::parse(value)
        .map_err(|error| WireSnapshotError::invalid("runtime entrypoint id", error))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePermissionRequest {
    permission: String,
    scope: WirePermissionScope,
    required: bool,
    reason: Option<String>,
}

impl WirePermissionRequest {
    fn capture(request: &PermissionRequest) -> Self {
        Self {
            permission: request.permission().as_str().to_owned(),
            scope: request.requested_scope().into(),
            required: request.required(),
            reason: request.reason().map(str::to_owned),
        }
    }

    fn into_domain(self) -> Result<PermissionRequest, WireSnapshotError> {
        Ok(PermissionRequest::new(
            PermissionId::parse(&self.permission)
                .map_err(|error| WireSnapshotError::invalid("permission request id", error))?,
            self.scope.into(),
            self.required,
            self.reason,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum WireContribution {
    Command {
        id: String,
        title: String,
        action: WireCommandAction,
    },
    Search {
        id: String,
        capability: String,
    },
}

impl WireContribution {
    fn capture(contribution: &ContributionDeclaration) -> Self {
        match contribution {
            ContributionDeclaration::Command(command) => Self::Command {
                id: command.id().as_str().to_owned(),
                title: command.title().to_owned(),
                action: WireCommandAction::capture(command.action()),
            },
            ContributionDeclaration::Search(search) => Self::Search {
                id: search.id().as_str().to_owned(),
                capability: search.capability().as_str().to_owned(),
            },
        }
    }

    fn into_domain(self) -> Result<ContributionDeclaration, WireSnapshotError> {
        match self {
            Self::Command { id, title, action } => Ok(CommandContributionDeclaration::new(
                ContributionId::parse(&id).map_err(|error| {
                    WireSnapshotError::invalid("command contribution id", error)
                })?,
                title,
                action.into_domain()?,
            )
            .map_err(|error| WireSnapshotError::invalid("command contribution", error))?
            .into()),
            Self::Search { id, capability } => Ok(rumahl_core::SearchContributionDeclaration::new(
                ContributionId::parse(&id)
                    .map_err(|error| WireSnapshotError::invalid("search contribution id", error))?,
                CapabilityId::parse(&capability).map_err(|error| {
                    WireSnapshotError::invalid("search contribution capability", error)
                })?,
            )
            .into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum WireCommandAction {
    InvokeCapability { capability: String },
    OpenApp { app_id: String },
}

impl WireCommandAction {
    fn capture(action: &CommandAction) -> Self {
        match action {
            CommandAction::InvokeCapability(capability) => Self::InvokeCapability {
                capability: capability.as_str().to_owned(),
            },
            CommandAction::OpenApp(app_id) => Self::OpenApp {
                app_id: app_id.as_str().to_owned(),
            },
        }
    }

    fn into_domain(self) -> Result<CommandAction, WireSnapshotError> {
        match self {
            Self::InvokeCapability { capability } => Ok(CommandAction::invoke_capability(
                CapabilityId::parse(&capability).map_err(|error| {
                    WireSnapshotError::invalid("command action capability", error)
                })?,
            )),
            Self::OpenApp { app_id } => Ok(CommandAction::open_app(
                AppId::parse(&app_id)
                    .map_err(|error| WireSnapshotError::invalid("command action app_id", error))?,
            )),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGrant {
    id: String,
    subject: WireIdentity,
    permission: String,
    scope: WirePermissionScope,
    resources: Vec<WireResource>,
    granted_by: WireIdentity,
}

impl WireGrant {
    fn capture(snapshot: &PermissionGrantSnapshot) -> Self {
        Self {
            id: snapshot.id().to_string(),
            subject: WireIdentity::capture(snapshot.subject()),
            permission: snapshot.permission().as_str().to_owned(),
            scope: snapshot.scope().into(),
            resources: snapshot
                .resources()
                .iter()
                .map(WireResource::capture)
                .collect(),
            granted_by: WireIdentity::capture(snapshot.granted_by()),
        }
    }

    fn into_domain(self) -> Result<PermissionGrantSnapshot, WireSnapshotError> {
        PermissionGrantSnapshot::new(
            GrantId::parse(&self.id)
                .map_err(|error| WireSnapshotError::invalid("grant id", error))?,
            self.subject.into_domain()?,
            PermissionId::parse(&self.permission)
                .map_err(|error| WireSnapshotError::invalid("grant permission", error))?,
            self.scope.into(),
            self.resources
                .into_iter()
                .map(WireResource::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            self.granted_by.into_domain()?,
        )
        .map_err(|error| WireSnapshotError::invalid("grant", error))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum WireIdentity {
    User {
        id: String,
    },
    App {
        app_id: String,
        installation_id: String,
        publisher_id: String,
    },
    Service {
        id: String,
    },
}

impl WireIdentity {
    fn capture(identity: &Identity) -> Self {
        match identity {
            Identity::User(user) => Self::User {
                id: user.id().to_string(),
            },
            Identity::App(app) => Self::App {
                app_id: app.app_id().as_str().to_owned(),
                installation_id: app.installation_id().to_string(),
                publisher_id: app.publisher_id().as_str().to_owned(),
            },
            Identity::Service(service) => Self::Service {
                id: service.id().as_str().to_owned(),
            },
        }
    }

    fn into_domain(self) -> Result<Identity, WireSnapshotError> {
        match self {
            Self::User { id } => Ok(UserIdentity::new(
                UserId::parse(&id)
                    .map_err(|error| WireSnapshotError::invalid("user identity", error))?,
            )
            .into()),
            Self::App {
                app_id,
                installation_id,
                publisher_id,
            } => Ok(AppIdentity::new(
                AppId::parse(&app_id)
                    .map_err(|error| WireSnapshotError::invalid("app identity app_id", error))?,
                InstallationId::parse(&installation_id).map_err(|error| {
                    WireSnapshotError::invalid("app identity installation_id", error)
                })?,
                PublisherId::parse(&publisher_id).map_err(|error| {
                    WireSnapshotError::invalid("app identity publisher_id", error)
                })?,
            )
            .into()),
            Self::Service { id } => Ok(ServiceIdentity::new(
                ServiceId::parse(&id)
                    .map_err(|error| WireSnapshotError::invalid("service identity", error))?,
            )
            .into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WirePermissionScope {
    AppPrivate,
    UserOwn,
    UserSelected,
    Explicit,
    FamilyShared,
    System,
}

impl From<PermissionScope> for WirePermissionScope {
    fn from(scope: PermissionScope) -> Self {
        match scope {
            PermissionScope::AppPrivate => Self::AppPrivate,
            PermissionScope::UserOwn => Self::UserOwn,
            PermissionScope::UserSelected => Self::UserSelected,
            PermissionScope::Explicit => Self::Explicit,
            PermissionScope::FamilyShared => Self::FamilyShared,
            PermissionScope::System => Self::System,
        }
    }
}

impl From<WirePermissionScope> for PermissionScope {
    fn from(scope: WirePermissionScope) -> Self {
        match scope {
            WirePermissionScope::AppPrivate => Self::AppPrivate,
            WirePermissionScope::UserOwn => Self::UserOwn,
            WirePermissionScope::UserSelected => Self::UserSelected,
            WirePermissionScope::Explicit => Self::Explicit,
            WirePermissionScope::FamilyShared => Self::FamilyShared,
            WirePermissionScope::System => Self::System,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResource {
    namespace: String,
    kind: String,
    key: String,
}

impl WireResource {
    fn capture(resource: &ResourceRef) -> Self {
        Self {
            namespace: resource.namespace().as_str().to_owned(),
            kind: resource.kind().as_str().to_owned(),
            key: resource.key().as_str().to_owned(),
        }
    }

    fn into_domain(self) -> Result<ResourceRef, WireSnapshotError> {
        Ok(ResourceRef::new(
            ResourceNamespace::parse(&self.namespace)
                .map_err(|error| WireSnapshotError::invalid("resource namespace", error))?,
            ResourceKind::parse(&self.kind)
                .map_err(|error| WireSnapshotError::invalid("resource kind", error))?,
            ResourceKey::parse(&self.key)
                .map_err(|error| WireSnapshotError::invalid("resource key", error))?,
        ))
    }
}
