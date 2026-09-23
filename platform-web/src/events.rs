use rumahl_core::UserId;
use rumahl_ui_contracts::ShellEvent;
use tokio::sync::broadcast;

pub type EventReceiver = broadcast::Receiver<(UserId, ShellEvent)>;

/// Replace this with a durable event feed without changing the WebSocket route.
pub trait ShellEventSource: Send + Sync + 'static {
    fn subscribe(&self) -> EventReceiver;
}

#[derive(Clone)]
pub struct InMemoryShellEvents {
    sender: broadcast::Sender<(UserId, ShellEvent)>,
}

impl InMemoryShellEvents {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn publish(&self, user_id: UserId, event: ShellEvent) {
        let _ = self.sender.send((user_id, event));
    }
}

impl ShellEventSource for InMemoryShellEvents {
    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}
