use crate::constants::SUBJECT_EMOJI_REGISTER;
use crate::handlers::register::handle_emoji_register;
use crate::store::ArcEmojiStore;
use crate::types::*;
use futures_util::StreamExt;
use std::sync::Arc;

pub async fn start_register_handler(
    client: Arc<async_nats::Client>,
    store: ArcEmojiStore,
) -> anyhow::Result<()> {
    let mut subscriber = client.subscribe(SUBJECT_EMOJI_REGISTER).await?;

    tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            let client = client.clone();
            if let Ok(request) = serde_json::from_slice::<EmojiRegisterRequest>(&message.payload) {
                let store = store.clone();
                tokio::spawn(async move {
                    match handle_emoji_register(request, store).await {
                        Ok(response) => {
                            let payload = serde_json::to_vec(&response).unwrap();
                            if let Some(subj) = message.reply {
                                if let Err(e) = client.publish(subj, payload.into()).await {
                                    tracing::error!(
                                        "Failed to send emoji registration response: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error handling emoji registration: {}", e);
                        }
                    }
                });
            } else {
                tracing::error!("Failed to deserialize message: {:?}", message.payload);
            }
        }
    });

    Ok(())
}
