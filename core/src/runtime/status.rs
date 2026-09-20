#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}
