use super::{AppId, InstallationId, PublisherId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppIdentity {
    app_id: AppId,
    installation_id: InstallationId,
    publisher_id: PublisherId,
}

impl AppIdentity {
    pub fn new(app_id: AppId, installation_id: InstallationId, publisher_id: PublisherId) -> Self {
        Self {
            app_id,
            installation_id,
            publisher_id,
        }
    }

    pub fn app_id(&self) -> &AppId {
        &self.app_id
    }

    pub fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    pub fn publisher_id(&self) -> &PublisherId {
        &self.publisher_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_app_identity() {
        let app_id = AppId::parse("com.rumahl.notes").unwrap();

        let publisher_id = PublisherId::parse("com.rumahl").unwrap();

        let installation_id = InstallationId::new();

        let identity = AppIdentity::new(app_id.clone(), installation_id, publisher_id.clone());

        assert_eq!(identity.app_id(), &app_id);
        assert_eq!(identity.publisher_id(), &publisher_id);
        assert_eq!(identity.installation_id(), &installation_id);
    }
}
