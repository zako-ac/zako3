use crate::metrics;
use imagehash::Hash;
use std::time::Instant;

pub async fn hash_image(url: &str) -> anyhow::Result<Hash> {
    let instant = Instant::now();
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;

    let img = tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes)?;
        let hash = imagehash::perceptual_hash(&image);

        anyhow::Result::<Hash>::Ok(hash)
    })
    .await??;

    let elapsed_secs = instant.elapsed().as_secs_f64();
    tracing::info!("Hashed image in {:.6} seconds", elapsed_secs);
    metrics::EMOJI_HASH_TIME.set(elapsed_secs);

    Ok(img)
}
