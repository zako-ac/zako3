use std::sync::Arc;

use serenity::{all::EventHandler, async_trait};
use tokio::sync::mpsc;

pub struct ReadyWaiter {
    ready_sender: mpsc::Sender<()>,
    ctx_sender: mpsc::Sender<serenity::all::Context>,
}

#[async_trait]
impl EventHandler for ReadyWaiter {
    async fn ready(&self, ctx: serenity::all::Context, ready: serenity::all::Ready) {
        tracing::info!("{} is connected!", ready.user.name);
        let _ = self.ctx_sender.send(ctx).await;
        let _ = self.ready_sender.send(()).await;
    }
}

pub fn create_ready_waiter() -> (
    Arc<ReadyWaiter>,
    mpsc::Receiver<()>,
    mpsc::Receiver<serenity::all::Context>,
) {
    let (ready_sender, ready_recv) = mpsc::channel(1);
    let (ctx_sender, ctx_recv) = mpsc::channel(1);
    let waiter = ReadyWaiter { ready_sender, ctx_sender };
    (Arc::new(waiter), ready_recv, ctx_recv)
}
