use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrantId(Uuid);

impl GrantId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for GrantId {
    fn default() -> Self {
        Self::new()
    }
}
