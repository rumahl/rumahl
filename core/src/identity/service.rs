use super::ServiceId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceIdentity {
    id: ServiceId,
}

impl ServiceIdentity {
    pub fn new(id: ServiceId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &ServiceId {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_service_identity() {
        let service_id =
            ServiceId::parse("rumahl.storage").unwrap();

        let identity =
            ServiceIdentity::new(service_id.clone());

        assert_eq!(identity.id(), &service_id);
    }
}