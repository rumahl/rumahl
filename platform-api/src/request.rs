use rumahl_core::OperationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRequest<T> {
    context: OperationContext,
    payload: T,
}

impl<T> PlatformRequest<T> {
    pub(crate) fn new(context: OperationContext, payload: T) -> Self {
        Self { context, payload }
    }

    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_parts(self) -> (OperationContext, T) {
        (self.context, self.payload)
    }
}
