use crate::metrics;
use crate::store::ArcEmojiStore;
use crate::types::*;
use crate::utils::image::hash_image;

pub async fn handle_emoji_register(
    request: EmojiRegisterRequest,
    store: ArcEmojiStore,
) -> anyhow::Result<EmojiRegisterResponse> {
    metrics::EMOJI_REGISTER_REQUESTS.inc();
    tracing::info!("Received emoji registration request: {:?}", request);

    tracing::info!(
        "Processing emoji: name={}, url={}",
        request.name,
        request.image_url
    );

    let hash = hash_image(&request.image_url).await?;

    let img_hash: ImgHash = hash.into();

    // match if exists
    if let Some(existing_emoji) = store.find_similar_emoji(&img_hash).await? {
        tracing::info!(
            "Emoji already exists: name={}, matched_name={}",
            request.name,
            existing_emoji.name
        );
        return Ok(EmojiRegisterResponse::AlreadyExists {
            matched_name: existing_emoji.name,
            matched_id: existing_emoji.registered_id,
        });
    }

    store
        .save_emoji(Emoji {
            hash: img_hash,
            registered_id: request.id.clone(),
            name: request.name.clone(),
        })
        .await?;

    Ok(EmojiRegisterResponse::Success)
}
