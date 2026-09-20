#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeKind {
    Web,
    Container,
    Native,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_kinds_are_distinct() {
        assert_ne!(RuntimeKind::Web, RuntimeKind::Container);
        assert_ne!(RuntimeKind::Container, RuntimeKind::Native);
        assert_ne!(RuntimeKind::Native, RuntimeKind::Web);
    }
}
