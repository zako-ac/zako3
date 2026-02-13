use crate::metrics;
use crate::store::ArcEmojiStore;
use crate::types::*;
use crate::utils::image::hash_image;
use std::sync::Arc;
use std::time::Instant;

pub const SUBJECT_EMOJI_MATCH: &str = "emoji.match";

pub async fn handle_emoji_match(
    request: EmojiMatchRequest,
    store: ArcEmojiStore,
    client: Arc<async_nats::Client>,
) -> anyhow::Result<()> {
    tracing::info!("Received emoji match request: {:?}", request);
    metrics::EMOJI_MATCH_REQUESTS.inc();

    tracing::info!(
        "Matching emoji: id={}, url={}",
        request.id,
        request.image_url
    );

    let instant = Instant::now();
    let hash = hash_image(&request.image_url).await?;
    tracing::info!(
        "Computed hash for image id={} in {:?}",
        request.id,
        instant.elapsed(),
    );

    if let Some(matched_emoji) = store.find_similar_emoji(&hash.into()).await? {
        tracing::info!(
            "Found matching emoji for id={}: name={}",
            request.id,
            matched_emoji.name
        );

        let notification = MatchedEmojiNotification {
            id: request.id,
            name: matched_emoji.name,
        };

        let payload = serde_json::to_vec(&notification)?;
        client.publish(SUBJECT_EMOJI_MATCH, payload.into()).await?;

        metrics::EMOJI_MATCH_HITS.inc();
    } else {
        tracing::info!("No matching emoji found for id={}", request.id);
    }

    Ok(())
}
