use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::{InstallationId, InstalledApp, InstalledAppSnapshot, UnixTimestamp};

use super::AppOperationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOperationKind {
    Install,
    Update,
    Uninstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOperationPhase {
    Applying,
    Committed,
    Compensating,
    Compensated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppOperationResource {
    AppDatabases,
    RuntimeInstance,
    OidcClient,
    RuntimeActivation,
    PlatformSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOperationResourceState {
    Pending,
    Applying,
    Applied,
    CompensationPending,
    Compensating,
    Compensated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOperationStep {
    resource: AppOperationResource,
    state: AppOperationResourceState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOperation {
    id: AppOperationId,
    installation_id: InstallationId,
    kind: AppOperationKind,
    phase: AppOperationPhase,
    steps: Vec<AppOperationStep>,
    revision: u64,
    started_at: UnixTimestamp,
    updated_at: UnixTimestamp,
    target_app: Option<InstalledAppSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppOperationError {
    EmptyPlan,
    DuplicateResource(AppOperationResource),
    PlatformSnapshotMissing,
    PlatformSnapshotMustBeLast,
    TimestampMovedBackwards,
    RevisionExhausted,
    WrongPhase {
        expected: AppOperationPhase,
        actual: AppOperationPhase,
    },
    ResourceNotFound(AppOperationResource),
    ResourceOutOfOrder(AppOperationResource),
    WrongResourceState {
        resource: AppOperationResource,
        expected: AppOperationResourceState,
        actual: AppOperationResourceState,
    },
    ResourcesNotApplied,
    CompensationIncomplete,
    InvalidRestoredState,
    TargetInstallationMismatch,
}

impl AppOperation {
    pub fn for_app(
        app: &InstalledApp,
        kind: AppOperationKind,
        started_at: UnixTimestamp,
    ) -> Result<Self, AppOperationError> {
        let mut resources = match kind {
            AppOperationKind::Install | AppOperationKind::Update => {
                let mut resources = Vec::new();
                if !app.manifest().databases().is_empty() {
                    resources.push(AppOperationResource::AppDatabases);
                }
                resources.push(AppOperationResource::RuntimeInstance);
                if app.manifest().oidc_client().is_some() {
                    resources.push(AppOperationResource::OidcClient);
                }
                resources.push(AppOperationResource::RuntimeActivation);
                resources
            }
            AppOperationKind::Uninstall => {
                let mut resources = vec![AppOperationResource::RuntimeActivation];
                if app.manifest().oidc_client().is_some() {
                    resources.push(AppOperationResource::OidcClient);
                }
                resources.push(AppOperationResource::RuntimeInstance);
                if !app.manifest().databases().is_empty() {
                    resources.push(AppOperationResource::AppDatabases);
                }
                resources
            }
        };

        resources.push(AppOperationResource::PlatformSnapshot);

        let mut operation = Self::new(*app.installation_id(), kind, resources, started_at)?;
        operation.target_app = Some(InstalledAppSnapshot::capture(app));
        Ok(operation)
    }

    pub fn new(
        installation_id: InstallationId,
        kind: AppOperationKind,
        resources: Vec<AppOperationResource>,
        started_at: UnixTimestamp,
    ) -> Result<Self, AppOperationError> {
        let steps = resources
            .into_iter()
            .map(|resource| AppOperationStep {
                resource,
                state: AppOperationResourceState::Pending,
            })
            .collect();

        Self::restore_with_target(
            AppOperationId::new(),
            installation_id,
            kind,
            AppOperationPhase::Applying,
            steps,
            0,
            started_at,
            started_at,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: AppOperationId,
        installation_id: InstallationId,
        kind: AppOperationKind,
        phase: AppOperationPhase,
        steps: Vec<AppOperationStep>,
        revision: u64,
        started_at: UnixTimestamp,
        updated_at: UnixTimestamp,
    ) -> Result<Self, AppOperationError> {
        Self::restore_with_target(
            id,
            installation_id,
            kind,
            phase,
            steps,
            revision,
            started_at,
            updated_at,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_for_app(
        id: AppOperationId,
        installation_id: InstallationId,
        kind: AppOperationKind,
        phase: AppOperationPhase,
        steps: Vec<AppOperationStep>,
        revision: u64,
        started_at: UnixTimestamp,
        updated_at: UnixTimestamp,
        target_app: InstalledAppSnapshot,
    ) -> Result<Self, AppOperationError> {
        Self::restore_with_target(
            id,
            installation_id,
            kind,
            phase,
            steps,
            revision,
            started_at,
            updated_at,
            Some(target_app),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn restore_with_target(
        id: AppOperationId,
        installation_id: InstallationId,
        kind: AppOperationKind,
        phase: AppOperationPhase,
        steps: Vec<AppOperationStep>,
        revision: u64,
        started_at: UnixTimestamp,
        updated_at: UnixTimestamp,
        target_app: Option<InstalledAppSnapshot>,
    ) -> Result<Self, AppOperationError> {
        Self::validate_plan(&steps)?;

        if updated_at < started_at {
            return Err(AppOperationError::TimestampMovedBackwards);
        }

        if !Self::phase_matches_steps(phase, &steps) {
            return Err(AppOperationError::InvalidRestoredState);
        }

        if target_app
            .as_ref()
            .is_some_and(|app| app.installation_id() != &installation_id)
        {
            return Err(AppOperationError::TargetInstallationMismatch);
        }

        Ok(Self {
            id,
            installation_id,
            kind,
            phase,
            steps,
            revision,
            started_at,
            updated_at,
            target_app,
        })
    }

    pub fn restored_step(
        resource: AppOperationResource,
        state: AppOperationResourceState,
    ) -> AppOperationStep {
        AppOperationStep { resource, state }
    }

    pub fn id(&self) -> &AppOperationId {
        &self.id
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn kind(&self) -> AppOperationKind {
        self.kind
    }

    pub fn phase(&self) -> AppOperationPhase {
        self.phase
    }

    pub fn steps(&self) -> &[AppOperationStep] {
        &self.steps
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn started_at(&self) -> UnixTimestamp {
        self.started_at
    }

    pub fn updated_at(&self) -> UnixTimestamp {
        self.updated_at
    }

    pub fn target_app(&self) -> Option<&InstalledAppSnapshot> {
        self.target_app.as_ref()
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.phase,
            AppOperationPhase::Committed | AppOperationPhase::Compensated
        )
    }

    pub fn begin_resource(
        &mut self,
        resource: AppOperationResource,
        at: UnixTimestamp,
    ) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Applying)?;
        let index = self.step_index(resource)?;

        if self.steps[..index]
            .iter()
            .any(|step| step.state != AppOperationResourceState::Applied)
        {
            return Err(AppOperationError::ResourceOutOfOrder(resource));
        }

        self.require_resource_state(index, AppOperationResourceState::Pending)?;
        self.advance(at)?;
        self.steps[index].state = AppOperationResourceState::Applying;
        Ok(())
    }

    pub fn complete_resource(
        &mut self,
        resource: AppOperationResource,
        at: UnixTimestamp,
    ) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Applying)?;
        let index = self.step_index(resource)?;
        self.require_resource_state(index, AppOperationResourceState::Applying)?;
        self.advance(at)?;
        self.steps[index].state = AppOperationResourceState::Applied;
        Ok(())
    }

    pub fn commit(&mut self, at: UnixTimestamp) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Applying)?;

        if self
            .steps
            .iter()
            .any(|step| step.state != AppOperationResourceState::Applied)
        {
            return Err(AppOperationError::ResourcesNotApplied);
        }

        self.advance(at)?;
        self.phase = AppOperationPhase::Committed;
        Ok(())
    }

    pub fn begin_compensation(&mut self, at: UnixTimestamp) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Applying)?;

        self.advance(at)?;
        for step in &mut self.steps {
            if matches!(
                step.state,
                AppOperationResourceState::Applying | AppOperationResourceState::Applied
            ) {
                step.state = AppOperationResourceState::CompensationPending;
            }
        }

        self.phase = AppOperationPhase::Compensating;
        Ok(())
    }

    pub fn begin_resource_compensation(
        &mut self,
        resource: AppOperationResource,
        at: UnixTimestamp,
    ) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Compensating)?;
        let index = self.step_index(resource)?;

        if self.steps[index + 1..].iter().any(|step| {
            matches!(
                step.state,
                AppOperationResourceState::CompensationPending
                    | AppOperationResourceState::Compensating
            )
        }) {
            return Err(AppOperationError::ResourceOutOfOrder(resource));
        }

        self.require_resource_state(index, AppOperationResourceState::CompensationPending)?;
        self.advance(at)?;
        self.steps[index].state = AppOperationResourceState::Compensating;
        Ok(())
    }

    pub fn complete_resource_compensation(
        &mut self,
        resource: AppOperationResource,
        at: UnixTimestamp,
    ) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Compensating)?;
        let index = self.step_index(resource)?;
        self.require_resource_state(index, AppOperationResourceState::Compensating)?;
        self.advance(at)?;
        self.steps[index].state = AppOperationResourceState::Compensated;
        Ok(())
    }

    pub fn finish_compensation(&mut self, at: UnixTimestamp) -> Result<(), AppOperationError> {
        self.require_phase(AppOperationPhase::Compensating)?;

        if self.steps.iter().any(|step| {
            !matches!(
                step.state,
                AppOperationResourceState::Pending | AppOperationResourceState::Compensated
            )
        }) {
            return Err(AppOperationError::CompensationIncomplete);
        }

        self.advance(at)?;
        self.phase = AppOperationPhase::Compensated;
        Ok(())
    }

    fn validate_plan(steps: &[AppOperationStep]) -> Result<(), AppOperationError> {
        if steps.is_empty() {
            return Err(AppOperationError::EmptyPlan);
        }

        let mut resources = HashSet::new();
        for step in steps {
            if !resources.insert(step.resource) {
                return Err(AppOperationError::DuplicateResource(step.resource));
            }
        }

        let Some(snapshot_index) = steps
            .iter()
            .position(|step| step.resource == AppOperationResource::PlatformSnapshot)
        else {
            return Err(AppOperationError::PlatformSnapshotMissing);
        };

        if snapshot_index + 1 != steps.len() {
            return Err(AppOperationError::PlatformSnapshotMustBeLast);
        }

        Ok(())
    }

    fn phase_matches_steps(phase: AppOperationPhase, steps: &[AppOperationStep]) -> bool {
        match phase {
            AppOperationPhase::Applying => Self::applying_steps_are_ordered(steps),
            AppOperationPhase::Committed => steps
                .iter()
                .all(|step| step.state == AppOperationResourceState::Applied),
            AppOperationPhase::Compensating => Self::compensating_steps_are_ordered(steps),
            AppOperationPhase::Compensated => Self::compensated_steps_are_ordered(steps),
        }
    }

    fn applying_steps_are_ordered(steps: &[AppOperationStep]) -> bool {
        let mut last_rank = 0;
        let mut applying_steps = 0;

        for step in steps {
            let rank = match step.state {
                AppOperationResourceState::Applied => 0,
                AppOperationResourceState::Applying => {
                    applying_steps += 1;
                    1
                }
                AppOperationResourceState::Pending => 2,
                AppOperationResourceState::CompensationPending
                | AppOperationResourceState::Compensating
                | AppOperationResourceState::Compensated => return false,
            };

            if rank < last_rank || applying_steps > 1 {
                return false;
            }
            last_rank = rank;
        }

        true
    }

    fn compensating_steps_are_ordered(steps: &[AppOperationStep]) -> bool {
        let mut last_rank = 0;
        let mut compensating_steps = 0;

        for step in steps {
            let rank = match step.state {
                AppOperationResourceState::CompensationPending => 0,
                AppOperationResourceState::Compensating => {
                    compensating_steps += 1;
                    1
                }
                AppOperationResourceState::Compensated => 2,
                AppOperationResourceState::Pending => 3,
                AppOperationResourceState::Applying | AppOperationResourceState::Applied => {
                    return false;
                }
            };

            if rank < last_rank || compensating_steps > 1 {
                return false;
            }
            last_rank = rank;
        }

        true
    }

    fn compensated_steps_are_ordered(steps: &[AppOperationStep]) -> bool {
        let mut pending_seen = false;

        for step in steps {
            match step.state {
                AppOperationResourceState::Compensated if !pending_seen => {}
                AppOperationResourceState::Pending => pending_seen = true,
                AppOperationResourceState::Compensated
                | AppOperationResourceState::Applying
                | AppOperationResourceState::Applied
                | AppOperationResourceState::CompensationPending
                | AppOperationResourceState::Compensating => return false,
            }
        }

        true
    }

    fn require_phase(&self, expected: AppOperationPhase) -> Result<(), AppOperationError> {
        if self.phase != expected {
            return Err(AppOperationError::WrongPhase {
                expected,
                actual: self.phase,
            });
        }

        Ok(())
    }

    fn step_index(&self, resource: AppOperationResource) -> Result<usize, AppOperationError> {
        self.steps
            .iter()
            .position(|step| step.resource == resource)
            .ok_or(AppOperationError::ResourceNotFound(resource))
    }

    fn require_resource_state(
        &self,
        index: usize,
        expected: AppOperationResourceState,
    ) -> Result<(), AppOperationError> {
        let step = &self.steps[index];

        if step.state != expected {
            return Err(AppOperationError::WrongResourceState {
                resource: step.resource,
                expected,
                actual: step.state,
            });
        }

        Ok(())
    }

    fn advance(&mut self, at: UnixTimestamp) -> Result<(), AppOperationError> {
        if at < self.updated_at {
            return Err(AppOperationError::TimestampMovedBackwards);
        }

        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(AppOperationError::RevisionExhausted)?;
        self.updated_at = at;
        Ok(())
    }
}

impl AppOperationStep {
    pub fn resource(&self) -> AppOperationResource {
        self.resource
    }

    pub fn state(&self) -> AppOperationResourceState {
        self.state
    }
}

impl fmt::Display for AppOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlan => write!(f, "app operation plan must not be empty"),
            Self::DuplicateResource(resource) => {
                write!(f, "app operation resource {resource:?} is duplicated")
            }
            Self::PlatformSnapshotMissing => {
                write!(f, "app operation must include the platform snapshot")
            }
            Self::PlatformSnapshotMustBeLast => {
                write!(
                    f,
                    "platform snapshot must be the final app operation resource"
                )
            }
            Self::TimestampMovedBackwards => write!(f, "app operation timestamp moved backwards"),
            Self::RevisionExhausted => write!(f, "app operation revision is exhausted"),
            Self::WrongPhase { expected, actual } => write!(
                f,
                "app operation phase must be {expected:?}, found {actual:?}"
            ),
            Self::ResourceNotFound(resource) => {
                write!(f, "app operation resource {resource:?} was not found")
            }
            Self::ResourceOutOfOrder(resource) => {
                write!(f, "app operation resource {resource:?} is out of order")
            }
            Self::WrongResourceState {
                resource,
                expected,
                actual,
            } => write!(
                f,
                "app operation resource {resource:?} must be {expected:?}, found {actual:?}"
            ),
            Self::ResourcesNotApplied => {
                write!(f, "not all app operation resources are applied")
            }
            Self::CompensationIncomplete => {
                write!(f, "app operation compensation is incomplete")
            }
            Self::InvalidRestoredState => write!(f, "restored app operation state is invalid"),
            Self::TargetInstallationMismatch => {
                write!(f, "app operation target has a different installation ID")
            }
        }
    }
}

impl Error for AppOperationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppDatabaseDeclaration, AppDatabaseId, AppId, AppManifest, AppManifestValidator,
        AppVersion, OidcCallbackPath, OidcClientDeclaration, OidcClientType, OidcScope,
        PackagePath, PublisherId, RuntimeDescriptor, RuntimeEntrypoint, RuntimeEntrypointId,
    };

    fn timestamp(value: u64) -> UnixTimestamp {
        UnixTimestamp::from_seconds(value)
    }

    fn operation() -> AppOperation {
        AppOperation::new(
            InstallationId::new(),
            AppOperationKind::Install,
            vec![
                AppOperationResource::AppDatabases,
                AppOperationResource::OidcClient,
                AppOperationResource::PlatformSnapshot,
            ],
            timestamp(10),
        )
        .unwrap()
    }

    fn installed_app(with_database: bool, with_oidc: bool) -> InstalledApp {
        let mut runtime = RuntimeDescriptor::web();
        runtime
            .add_entrypoint(RuntimeEntrypoint::web_asset(
                RuntimeEntrypointId::parse("main").unwrap(),
                PackagePath::parse("frontend/index.html").unwrap(),
            ))
            .unwrap();
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
            runtime,
        )
        .unwrap();

        if with_database {
            manifest
                .add_database(AppDatabaseDeclaration::new(
                    AppDatabaseId::parse("primary").unwrap(),
                ))
                .unwrap();
        }

        if with_oidc {
            manifest
                .declare_oidc_client(
                    OidcClientDeclaration::new(
                        OidcClientType::Public,
                        RuntimeEntrypointId::parse("main").unwrap(),
                        OidcCallbackPath::parse("/auth/callback").unwrap(),
                        vec![OidcScope::OpenId],
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        InstalledApp::create(manifest, &AppManifestValidator::new()).unwrap()
    }

    #[test]
    fn app_plan_includes_only_declared_optional_resources() {
        let neither = AppOperation::for_app(
            &installed_app(false, false),
            AppOperationKind::Install,
            timestamp(10),
        )
        .unwrap();
        assert_eq!(
            neither
                .steps()
                .iter()
                .map(AppOperationStep::resource)
                .collect::<Vec<_>>(),
            vec![
                AppOperationResource::RuntimeInstance,
                AppOperationResource::RuntimeActivation,
                AppOperationResource::PlatformSnapshot,
            ]
        );
        assert!(neither.target_app().is_some());

        let both = AppOperation::for_app(
            &installed_app(true, true),
            AppOperationKind::Install,
            timestamp(10),
        )
        .unwrap();
        assert_eq!(
            both.steps()
                .iter()
                .map(AppOperationStep::resource)
                .collect::<Vec<_>>(),
            vec![
                AppOperationResource::AppDatabases,
                AppOperationResource::RuntimeInstance,
                AppOperationResource::OidcClient,
                AppOperationResource::RuntimeActivation,
                AppOperationResource::PlatformSnapshot,
            ]
        );
        assert_eq!(
            both.target_app().unwrap().installation_id(),
            both.installation_id()
        );
    }

    #[test]
    fn uninstall_plan_stops_runtime_before_revoking_secrets_and_removing_resources() {
        let operation = AppOperation::for_app(
            &installed_app(true, true),
            AppOperationKind::Uninstall,
            timestamp(10),
        )
        .unwrap();

        assert_eq!(
            operation
                .steps()
                .iter()
                .map(AppOperationStep::resource)
                .collect::<Vec<_>>(),
            vec![
                AppOperationResource::RuntimeActivation,
                AppOperationResource::OidcClient,
                AppOperationResource::RuntimeInstance,
                AppOperationResource::AppDatabases,
                AppOperationResource::PlatformSnapshot,
            ]
        );
    }

    #[test]
    fn applies_resources_in_plan_order_and_commits() {
        let mut operation = operation();

        for (offset, resource) in [
            AppOperationResource::AppDatabases,
            AppOperationResource::OidcClient,
            AppOperationResource::PlatformSnapshot,
        ]
        .into_iter()
        .enumerate()
        {
            operation
                .begin_resource(resource, timestamp(11 + offset as u64 * 2))
                .unwrap();
            operation
                .complete_resource(resource, timestamp(12 + offset as u64 * 2))
                .unwrap();
        }

        operation.commit(timestamp(17)).unwrap();

        assert_eq!(operation.phase(), AppOperationPhase::Committed);
        assert_eq!(operation.revision(), 7);
        assert!(operation.is_terminal());
    }

    #[test]
    fn leaves_in_progress_step_visible_for_crash_recovery() {
        let mut operation = operation();

        operation
            .begin_resource(AppOperationResource::AppDatabases, timestamp(11))
            .unwrap();

        assert_eq!(
            operation.steps()[0].state(),
            AppOperationResourceState::Applying
        );
        assert!(!operation.is_terminal());
    }

    #[test]
    fn rejects_applying_resources_out_of_order() {
        let mut operation = operation();

        assert_eq!(
            operation
                .begin_resource(AppOperationResource::OidcClient, timestamp(11))
                .unwrap_err(),
            AppOperationError::ResourceOutOfOrder(AppOperationResource::OidcClient)
        );
        assert_eq!(operation.revision(), 0);
    }

    #[test]
    fn failed_transition_keeps_operation_unchanged() {
        let mut operation = operation();
        let before = operation.clone();

        assert_eq!(
            operation
                .begin_resource(AppOperationResource::AppDatabases, timestamp(9))
                .unwrap_err(),
            AppOperationError::TimestampMovedBackwards
        );
        assert_eq!(operation, before);
    }

    #[test]
    fn compensates_touched_resources_in_reverse_order() {
        let mut operation = operation();
        operation
            .begin_resource(AppOperationResource::AppDatabases, timestamp(11))
            .unwrap();
        operation
            .complete_resource(AppOperationResource::AppDatabases, timestamp(12))
            .unwrap();
        operation
            .begin_resource(AppOperationResource::OidcClient, timestamp(13))
            .unwrap();
        operation.begin_compensation(timestamp(14)).unwrap();

        assert_eq!(
            operation
                .begin_resource_compensation(AppOperationResource::AppDatabases, timestamp(15),)
                .unwrap_err(),
            AppOperationError::ResourceOutOfOrder(AppOperationResource::AppDatabases)
        );

        operation
            .begin_resource_compensation(AppOperationResource::OidcClient, timestamp(15))
            .unwrap();
        operation
            .complete_resource_compensation(AppOperationResource::OidcClient, timestamp(16))
            .unwrap();
        operation
            .begin_resource_compensation(AppOperationResource::AppDatabases, timestamp(17))
            .unwrap();
        operation
            .complete_resource_compensation(AppOperationResource::AppDatabases, timestamp(18))
            .unwrap();
        operation.finish_compensation(timestamp(19)).unwrap();

        assert_eq!(operation.phase(), AppOperationPhase::Compensated);
        assert!(operation.is_terminal());
        assert_eq!(
            operation.steps()[2].state(),
            AppOperationResourceState::Pending
        );
    }

    #[test]
    fn requires_platform_snapshot_as_final_step() {
        let installation_id = InstallationId::new();

        assert_eq!(
            AppOperation::new(
                installation_id,
                AppOperationKind::Install,
                vec![AppOperationResource::AppDatabases],
                timestamp(10),
            )
            .unwrap_err(),
            AppOperationError::PlatformSnapshotMissing
        );
        assert_eq!(
            AppOperation::new(
                installation_id,
                AppOperationKind::Install,
                vec![
                    AppOperationResource::PlatformSnapshot,
                    AppOperationResource::OidcClient,
                ],
                timestamp(10),
            )
            .unwrap_err(),
            AppOperationError::PlatformSnapshotMustBeLast
        );
    }

    #[test]
    fn rejects_invalid_restored_terminal_state() {
        let result = AppOperation::restore(
            AppOperationId::new(),
            InstallationId::new(),
            AppOperationKind::Install,
            AppOperationPhase::Committed,
            vec![AppOperation::restored_step(
                AppOperationResource::PlatformSnapshot,
                AppOperationResourceState::Pending,
            )],
            0,
            timestamp(10),
            timestamp(10),
        );

        assert_eq!(result.unwrap_err(), AppOperationError::InvalidRestoredState);
    }

    #[test]
    fn rejects_out_of_order_restored_steps() {
        let result = AppOperation::restore(
            AppOperationId::new(),
            InstallationId::new(),
            AppOperationKind::Install,
            AppOperationPhase::Applying,
            vec![
                AppOperation::restored_step(
                    AppOperationResource::AppDatabases,
                    AppOperationResourceState::Pending,
                ),
                AppOperation::restored_step(
                    AppOperationResource::PlatformSnapshot,
                    AppOperationResourceState::Applied,
                ),
            ],
            2,
            timestamp(10),
            timestamp(12),
        );

        assert_eq!(result.unwrap_err(), AppOperationError::InvalidRestoredState);
    }
}
