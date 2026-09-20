use super::RuntimeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDescriptor {
    kind: RuntimeKind,
}

impl RuntimeDescriptor {
    pub fn new(kind: RuntimeKind) -> Self {
        Self { kind }
    }

    pub fn web() -> Self {
        Self::new(RuntimeKind::Web)
    }

    pub fn container() -> Self {
        Self::new(RuntimeKind::Container)
    }

    pub fn native() -> Self {
        Self::new(RuntimeKind::Native)
    }

    pub fn kind(&self) -> RuntimeKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_web_runtime_descriptor() {
        assert_eq!(RuntimeDescriptor::web().kind(), RuntimeKind::Web);
    }

    #[test]
    fn creates_container_runtime_descriptor() {
        assert_eq!(
            RuntimeDescriptor::container().kind(),
            RuntimeKind::Container
        );
    }

    #[test]
    fn creates_native_runtime_descriptor() {
        assert_eq!(RuntimeDescriptor::native().kind(), RuntimeKind::Native);
    }
}
