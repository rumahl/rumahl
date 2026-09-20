use std::fmt;

use crate::{AppIdentity, UnixTimestamp};

use super::{SecretId, SecretPurpose, SecretValue};

pub struct SecretRecord {
    id: SecretId,
    owner: AppIdentity,
    purpose: SecretPurpose,
    value: SecretValue,
    created_at: UnixTimestamp,
}

impl SecretRecord {
    pub fn new(
        owner: AppIdentity,
        purpose: SecretPurpose,
        value: SecretValue,
        created_at: UnixTimestamp,
    ) -> Self {
        Self::restore(SecretId::new(), owner, purpose, value, created_at)
    }

    pub fn restore(
        id: SecretId,
        owner: AppIdentity,
        purpose: SecretPurpose,
        value: SecretValue,
        created_at: UnixTimestamp,
    ) -> Self {
        Self {
            id,
            owner,
            purpose,
            value,
            created_at,
        }
    }

    pub fn id(&self) -> &SecretId {
        &self.id
    }

    pub fn owner(&self) -> &AppIdentity {
        &self.owner
    }

    pub fn purpose(&self) -> &SecretPurpose {
        &self.purpose
    }

    pub fn value(&self) -> &SecretValue {
        &self.value
    }

    pub fn created_at(&self) -> UnixTimestamp {
        self.created_at
    }
}

impl fmt::Debug for SecretRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRecord")
            .field("id", &self.id)
            .field("owner", &self.owner)
            .field("purpose", &self.purpose)
            .field("value", &self.value)
            .field("created_at", &self.created_at)
            .finish()
    }
}
