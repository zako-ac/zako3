use std::sync::Arc;

use serenity::{all::EventHandler, async_trait};
use tokio::sync::mpsc;

pub struct ReadyWaiter {
    sender: mpsc::Sender<()>,
}

#[async_trait]
impl EventHandler for ReadyWaiter {
    async fn ready(&self, _: serenity::all::Context, ready: serenity::all::Ready) {
        let _ = self.sender.send(()).await;
        tracing::info!("{} is connected!", ready.user.name);
    }
}

pub fn create_ready_waiter() -> (Arc<ReadyWaiter>, mpsc::Receiver<()>) {
    let (sender, receiver) = mpsc::channel(1);
    let waiter = ReadyWaiter { sender };

    (Arc::new(waiter), receiver)
}
