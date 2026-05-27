use std::sync::Arc;

use tokio::sync::mpsc;
use zako3_emoji_matcher_proto::EmojiScopeMatchRequest;

use crate::handlers::scope_match::{ScopeMatchContext, handle_scope_match};
use crate::metrics;

#[derive(Clone)]
pub struct TaskQueue {
    tx: mpsc::Sender<EmojiScopeMatchRequest>,
}

impl TaskQueue {
    pub fn spawn(ctx: Arc<ScopeMatchContext>, concurrency: usize, capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel::<EmojiScopeMatchRequest>(capacity);
        let rx = Arc::new(tokio::sync::Mutex::new(rx));

        for runner_id in 0..concurrency {
            let ctx = ctx.clone();
            let rx = rx.clone();
            tokio::spawn(async move {
                tracing::info!(runner_id, "scope-match runner started");
                loop {
                    let req = {
                        let mut guard = rx.lock().await;
                        match guard.recv().await {
                            Some(req) => req,
                            None => break,
                        }
                    };
                    if let Err(e) = handle_scope_match(req, ctx.clone()).await {
                        tracing::warn!(runner_id, error = %e, "scope_match handler failed");
                    }
                }
                tracing::info!(runner_id, "scope-match runner stopped");
            });
        }

        Self { tx }
    }

    /// Push a task; drops with a warn if the queue is at capacity.
    pub fn submit(&self, req: EmojiScopeMatchRequest) {
        match self.tx.try_send(req) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(req)) => {
                metrics::EMOJI_SCOPE_MATCH_DROPS.inc();
                tracing::warn!(
                    emoji_id = %req.emoji_id,
                    "task queue full; dropping scope-match request"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!("task queue closed; cannot submit scope-match request");
            }
        }
    }
}
