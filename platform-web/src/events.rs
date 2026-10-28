use rumahl_core::UserId;
use rumahl_ui_contracts::ShellEvent;
use tokio::sync::broadcast;

/// A message on the shell event bus: either targeted at one user, or a system
/// lifecycle signal that every connected client must see.
#[derive(Clone)]
pub enum ShellEventMessage {
    ForUser(UserId, ShellEvent),
    ForAll(ShellEvent),
}

pub type EventReceiver = broadcast::Receiver<ShellEventMessage>;

/// Replace this with a durable event feed without changing the WebSocket route.
pub trait ShellEventSource: Send + Sync + 'static {
    fn subscribe(&self) -> EventReceiver;
}

#[derive(Clone)]
pub struct InMemoryShellEvents {
    sender: broadcast::Sender<ShellEventMessage>,
}

impl InMemoryShellEvents {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn publish(&self, user_id: UserId, event: ShellEvent) {
        let _ = self.sender.send(ShellEventMessage::ForUser(user_id, event));
    }

    /// Deliver an event to every connected client, regardless of account.
    pub fn publish_all(&self, event: ShellEvent) {
        let _ = self.sender.send(ShellEventMessage::ForAll(event));
    }
}

impl ShellEventSource for InMemoryShellEvents {
    fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}
