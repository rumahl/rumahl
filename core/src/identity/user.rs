use super::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserIdentity {
    id: UserId,
}

impl UserIdentity {
    pub fn new(id: UserId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_user_identity() {
        let user_id = UserId::new();

        let identity = UserIdentity::new(user_id);

        assert_eq!(identity.id(), &user_id);
    }
}
