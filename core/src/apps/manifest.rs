use std::error::Error;
use std::fmt;

use crate::{AppId, PublisherId};

use super::{AppVersion, ContributionDeclaration};

use crate::CapabilityId;
use crate::PermissionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    app_id: AppId,
    publisher_id: PublisherId,
    version: AppVersion,
    display_name: String,
    permission_requests: Vec<PermissionRequest>,
    provided_capabilities: Vec<CapabilityId>,
    contributions: Vec<ContributionDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManifestError {
    EmptyDisplayName,
    DisplayNameTooLong,
    DuplicatePermissionRequest,
    DuplicateProvidedCapability,
    DuplicateContribution,
}

impl AppManifest {
    pub fn new(
        app_id: AppId,
        publisher_id: PublisherId,
        version: AppVersion,
        display_name: impl Into<String>,
    ) -> Result<Self, AppManifestError> {
        let display_name = display_name.into();

        let display_name = display_name.trim();

        if display_name.is_empty() {
            return Err(AppManifestError::EmptyDisplayName);
        }

        if display_name.chars().count() > 120 {
            return Err(AppManifestError::DisplayNameTooLong);
        }

        Ok(Self {
            app_id,
            publisher_id,
            version,
            display_name: display_name.to_owned(),
            permission_requests: Vec::new(),
            provided_capabilities: Vec::new(),
            contributions: Vec::new(),
        })
    }

    pub fn app_id(&self) -> &AppId {
        &self.app_id
    }

    pub fn publisher_id(&self) -> &PublisherId {
        &self.publisher_id
    }

    pub fn version(&self) -> &AppVersion {
        &self.version
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn add_permission_request(
        &mut self,
        request: PermissionRequest,
    ) -> Result<(), AppManifestError> {
        if self
            .permission_requests
            .iter()
            .any(|existing| existing.permission() == request.permission())
        {
            return Err(AppManifestError::DuplicatePermissionRequest);
        }

        self.permission_requests.push(request);

        Ok(())
    }

    pub fn permission_requests(&self) -> &[PermissionRequest] {
        &self.permission_requests
    }

    pub fn add_provided_capability(
        &mut self,
        capability: CapabilityId,
    ) -> Result<(), AppManifestError> {
        if self
            .provided_capabilities
            .iter()
            .any(|existing| existing == &capability)
        {
            return Err(AppManifestError::DuplicateProvidedCapability);
        }

        self.provided_capabilities.push(capability);

        Ok(())
    }

    pub fn provided_capabilities(&self) -> &[CapabilityId] {
        &self.provided_capabilities
    }

    pub fn add_contribution(
        &mut self,
        contribution: ContributionDeclaration,
    ) -> Result<(), AppManifestError> {
        if self
            .contributions
            .iter()
            .any(|existing| existing.id() == contribution.id())
        {
            return Err(AppManifestError::DuplicateContribution);
        }

        self.contributions.push(contribution);

        Ok(())
    }

    pub fn contributions(&self) -> &[ContributionDeclaration] {
        &self.contributions
    }
}

impl fmt::Display for AppManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDisplayName => {
                write!(f, "app display name cannot be empty")
            }

            Self::DisplayNameTooLong => {
                write!(f, "app display name cannot exceed 120 characters")
            }
            Self::DuplicatePermissionRequest => {
                write!(
                    f,
                    "app manifest cannot request the same permission more than once"
                )
            }
            Self::DuplicateProvidedCapability => {
                write!(
                    f,
                    "app manifest cannot provide the same capability more than once"
                )
            }

            Self::DuplicateContribution => {
                write!(
                    f,
                    "app manifest cannot declare the same contribution id more than once"
                )
            }
        }
    }
}

impl Error for AppManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::CapabilityId;
    use crate::{PermissionId, PermissionRequest, PermissionScope};

    #[test]
    fn creates_app_manifest() {
        let app_id = AppId::parse("com.rumahl.notes").unwrap();

        let publisher_id = PublisherId::parse("com.rumahl").unwrap();

        let version = AppVersion::new(1, 4, 2);

        let manifest =
            AppManifest::new(app_id.clone(), publisher_id.clone(), version, "Notes").unwrap();

        assert_eq!(manifest.app_id(), &app_id);

        assert_eq!(manifest.publisher_id(), &publisher_id);

        assert_eq!(manifest.version(), &AppVersion::new(1, 4, 2,));

        assert_eq!(manifest.display_name(), "Notes");
    }

    #[test]
    fn trims_display_name() {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "   Notes   ",
        )
        .unwrap();

        assert_eq!(manifest.display_name(), "Notes");
    }

    #[test]
    fn rejects_empty_display_name() {
        let result = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "   ",
        );

        assert_eq!(result.unwrap_err(), AppManifestError::EmptyDisplayName);
    }

    #[test]
    fn rejects_display_name_longer_than_limit() {
        let result = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "a".repeat(121),
        );

        assert_eq!(result.unwrap_err(), AppManifestError::DisplayNameTooLong);
    }

    #[test]
    fn app_manifest_starts_without_permission_requests() {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        assert!(manifest.permission_requests().is_empty());
    }

    #[test]
    fn app_manifest_can_request_permission() {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        let permission = PermissionId::parse("rumahl.files.read").unwrap();

        manifest
            .add_permission_request(PermissionRequest::new(
                permission.clone(),
                PermissionScope::UserSelected,
                true,
                Some("Open files selected by the user".to_owned()),
            ))
            .unwrap();

        assert_eq!(manifest.permission_requests().len(), 1);

        let request = &manifest.permission_requests()[0];

        assert_eq!(request.permission(), &permission);

        assert_eq!(request.requested_scope(), PermissionScope::UserSelected);

        assert!(request.required());

        assert_eq!(request.reason(), Some("Open files selected by the user"));
    }

    #[test]
    fn rejects_duplicate_permission_request() {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        let permission = PermissionId::parse("rumahl.files.read").unwrap();

        manifest
            .add_permission_request(PermissionRequest::new(
                permission.clone(),
                PermissionScope::UserSelected,
                true,
                None,
            ))
            .unwrap();

        let result = manifest.add_permission_request(PermissionRequest::new(
            permission,
            PermissionScope::System,
            false,
            None,
        ));

        assert_eq!(
            result.unwrap_err(),
            AppManifestError::DuplicatePermissionRequest
        );
    }

    #[test]
    fn app_manifest_starts_without_provided_capabilities() {
        let manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        assert!(manifest.provided_capabilities().is_empty());
    }

    #[test]
    fn app_manifest_can_provide_capability() {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        manifest
            .add_provided_capability(capability.clone())
            .unwrap();

        assert_eq!(manifest.provided_capabilities(), &[capability]);
    }

    #[test]
    fn app_manifest_can_provide_multiple_capabilities() {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        manifest
            .add_provided_capability(CapabilityId::parse("rumahl.search.query").unwrap())
            .unwrap();

        manifest
            .add_provided_capability(CapabilityId::parse("com.rumahl.notes.create-note").unwrap())
            .unwrap();

        assert_eq!(manifest.provided_capabilities().len(), 2);
    }

    #[test]
    fn rejects_duplicate_provided_capability() {
        let mut manifest = AppManifest::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            PublisherId::parse("com.rumahl").unwrap(),
            AppVersion::new(1, 0, 0),
            "Notes",
        )
        .unwrap();

        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        manifest
            .add_provided_capability(capability.clone())
            .unwrap();

        let result = manifest.add_provided_capability(capability);

        assert_eq!(
            result.unwrap_err(),
            AppManifestError::DuplicateProvidedCapability
        );
    }
}
