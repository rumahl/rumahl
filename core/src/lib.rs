pub mod accounts;
pub mod app_operations;
pub mod apps;
pub mod capabilities;
pub mod context;
pub mod contributions;
pub mod databases;
pub mod events;
pub mod identity;
pub mod permissions;
pub mod persistence;
pub mod platform;
pub mod resources;
pub mod runtime;
pub mod secrets;

pub use accounts::{
    AccountRegistry, AccountRegistryError, AccountSession, AccountSessionError,
    AccountSessionRegistry, AccountSessionRegistryError, AccountState, AccountStateError,
    AccountStateRepository, AccountStatus, AccountUsername, AccountUsernameError, LocalAccount,
    LocalAccountError, UnixTimestamp, UnixTimestampError,
};

pub use app_operations::{
    AppOperation, AppOperationError, AppOperationId, AppOperationIdError, AppOperationKind,
    AppOperationPhase, AppOperationRepository, AppOperationResource, AppOperationResourceState,
    AppOperationStep,
};

pub use apps::{
    AppLifecycle, AppLifecycleError, AppManifest, AppManifestError, AppManifestValidationError,
    AppManifestValidator, AppSettingDeclaration, AppSettingError, AppSettingKey, AppSettingKind,
    AppSettingOption, AppUninstallResult, AppVersion, AppVersionError,
    CommandContributionDeclaration, CommandContributionDeclarationError, ConnectorDeclaration,
    ConnectorTarget, ConnectorTargetError, ContributionDeclaration,
    InstalledApp, InstalledAppError, InstalledAppRegistry, InstalledAppRegistryError,
    MAX_APP_SETTINGS, OidcCallbackPath, OidcCallbackPathError, OidcClientDeclaration,
    OidcClientDeclarationError, OidcClientType, OidcScope, PlatformDeregistrationReport,
    PlatformRegistrar, PlatformRegistrarError, PlatformRegistration, PlatformRegistrationError,
    PreferredStreamSize, RuntimeLifecycle, STREAMING_ENGINE_CAPABILITY, SearchContributionDeclaration,
    StreamPresentation, StreamPresentationError,
};

pub use context::{CorrelationId, CorrelationIdError, OperationContext};

pub use databases::{
    AppDatabaseBinding, AppDatabaseDeclaration, AppDatabaseId, AppDatabaseIdError,
    AppDatabaseInstallationState, AppDatabaseProvider, AppDatabaseRegistry,
    AppDatabaseRegistryError,
};

pub use identity::{
    AppId, AppIdError, AppIdentity, Identity, InstallationId, InstallationIdError, PublisherId,
    PublisherIdError, ServiceId, ServiceIdError, ServiceIdentity, SessionId, SessionIdError,
    UserId, UserIdError, UserIdentity,
};

pub use resources::{
    ResourceKey, ResourceKeyError, ResourceKind, ResourceKindError, ResourceNamespace,
    ResourceNamespaceError, ResourceRef,
};

pub use secrets::{
    SecretId, SecretIdError, SecretPurpose, SecretPurposeError, SecretRecord, SecretStore,
    SecretValue, SecretValueError,
};

pub use runtime::{
    AppRuntimeInstallationState, AppRuntimeProvider, PackagePath, PackagePathError, RuntimeAdapter,
    RuntimeAdapterError, RuntimeAdapterRegistry, RuntimeAdapterRegistryError, RuntimeController,
    RuntimeDescriptor,
    RuntimeDescriptorError, RuntimeEndpointId, RuntimeEndpointIdError, RuntimeEntrypoint,
    RuntimeEntrypointId, RuntimeEntrypointIdError, RuntimeEntrypointKind, RuntimeEntrypointTarget,
    RuntimeKind, RuntimeRouter, RuntimeRoutingError, RuntimeStatus, ProviderRuntimeAdapter,
    RuntimeChannel, RuntimeChannelError, RuntimeChannelRegistry, RuntimeEvent,
};

pub use permissions::{
    AuthorizationDecision, AuthorizationDenyReason, AuthorizationEngine, AuthorizationRequest,
    GrantAuthority, GrantAuthorityError, GrantId, GrantIdError, GrantIssuerPolicy,
    GrantIssuerPolicyError, InMemoryGrantStore, PermissionGrant, PermissionGrantError,
    PermissionId, PermissionIdError, PermissionRequest, PermissionScope, UserRole,
};

pub use persistence::{
    InstalledAppSnapshot, PLATFORM_SNAPSHOT_VERSION, PermissionGrantSnapshot, PlatformLoadError,
    PlatformPersistence, PlatformRecovery, PlatformRecoveryError, PlatformRecoveryReport,
    PlatformSnapshot, PlatformSnapshotRepository,
};

pub use capabilities::{
    CapabilityAccessRegistry, CapabilityAccessRegistryError, CapabilityAccessRule,
    CapabilityAccessRuleError, CapabilityDispatchError, CapabilityDispatchOutcome,
    CapabilityDispatcher, CapabilityExecution, CapabilityId, CapabilityIdError,
    CapabilityInvocation, CapabilityProvider, CapabilityProviderError, CapabilityRegistry,
    CapabilityRegistryError,
};

pub use contributions::{
    CommandAction, CommandContribution, CommandContributionError, CommandRegistry,
    CommandRegistryError, Contribution, ContributionError, ContributionId, ContributionIdError,
    ContributionKind, ContributionKindError, ContributionRegistry, ContributionRegistryError,
    SearchContribution, SearchProviderResolutionError, SearchProviderResolver, SearchRegistry,
    SearchRegistryError,
};

pub use events::{
    EventBus, EventBusError, EventDelivery, EventEnvelope, EventId, EventName, EventNameError,
    EventSubscription, EventSubscriptionError,
};

pub use platform::{
    AuditAction, AuditActor, AuditError, AuditEvent, AuditLog, AuditOutcome, MAX_TARGET_BYTES,
    NewAuditEvent, OsMode, OsModeError, OsModePolicy, OsModeRepository, OsModeSettings,
    OsModeStoreError, PlatformState,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_explicit_file_permission_grant() {
        let app = AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        );

        let file = ResourceRef::new(
            ResourceNamespace::parse("rumahl.files").unwrap(),
            ResourceKind::parse("file").unwrap(),
            ResourceKey::parse("note.md").unwrap(),
        );

        let subject = app.clone().into();

        let grant = PermissionGrant::new(
            subject,
            PermissionId::parse("rumahl.files.read").unwrap(),
            PermissionScope::Explicit,
            vec![file.clone()],
            app.into(),
        )
        .unwrap();

        assert_eq!(grant.permission().as_str(), "rumahl.files.read");

        assert_eq!(grant.resources(), &[file]);
    }
}

mod shell_preferences;
pub use shell_preferences::{
    BrowserProfileId, DEFAULT_THEME_ID, PreferenceScope, ShellMode, ShellPreferenceUpdate,
    ShellPreferences, ShellPreferencesError, ShellPreferencesRepository, valid_theme_id,
};
mod workspace;
pub use workspace::{WorkspacePreferences, WorkspaceRepository};
mod personal_files;
pub use personal_files::{FileError, PersonalFile, PersonalFiles};
