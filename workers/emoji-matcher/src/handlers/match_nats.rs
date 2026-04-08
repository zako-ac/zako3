use crate::constants::SUBJECT_EMOJI_MATCH;
use crate::handlers::match_emoji::handle_emoji_match;
use crate::store::ArcEmojiStore;
use crate::types::*;
use futures_util::StreamExt;
use std::sync::Arc;

pub async fn start_match_handler(
    client: Arc<async_nats::Client>,
    store: ArcEmojiStore,
) -> anyhow::Result<()> {
    let mut subscriber = client.subscribe(SUBJECT_EMOJI_MATCH).await?;

    tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            if let Ok(request) = serde_json::from_slice::<EmojiMatchRequest>(&message.payload) {
                let store = store.clone();
                let client = client.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_emoji_match(request, store, client).await {
                        tracing::error!("Error handling emoji match: {}", e);
                    }
                });
            } else {
                tracing::error!("Failed to deserialize message: {:?}", message.payload);
            }
        }
    });

    Ok(())
}
