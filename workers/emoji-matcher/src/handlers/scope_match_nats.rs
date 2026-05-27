use std::sync::Arc;

use futures_util::StreamExt;
use zako3_emoji_matcher_proto::{EmojiScopeMatchRequest, SUBJECT_EMOJI_SCOPE_MATCH};

use crate::task_queue::TaskQueue;

pub async fn start_scope_match_handler(
    client: Arc<async_nats::Client>,
    queue: TaskQueue,
) -> anyhow::Result<()> {
    let mut subscriber = client.subscribe(SUBJECT_EMOJI_SCOPE_MATCH).await?;

    tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            match serde_json::from_slice::<EmojiScopeMatchRequest>(&message.payload) {
                Ok(req) => queue.submit(req),
                Err(e) => tracing::warn!(error = %e, "failed to decode scope-match request"),
            }
        }
    });

    Ok(())
}
