use std::sync::Arc;

use zako3_emoji_matcher_proto::{EmojiScopeMatchRequest, SUBJECT_EMOJI_SCOPE_MATCH};

use crate::{CoreError, CoreResult};

/// Fire-and-forget publisher for the emoji-matcher worker.
///
/// HQ does not wait for a reply — the worker runs the scope-scan asynchronously
/// and writes new emoji mappings directly into the settings tables. The first
/// time an unknown emoji is seen it stays unmapped (the TTS pipeline falls back
/// to its default "Emoji" reading); subsequent uses will be mapped.
#[derive(Clone)]
pub struct EmojiMatchPublisher {
    client: Arc<async_nats::Client>,
}

impl EmojiMatchPublisher {
    pub async fn connect(nats_url: &str) -> CoreResult<Self> {
        let client = async_nats::connect(nats_url)
            .await
            .map_err(|e| CoreError::Internal(format!("NATS connect failed: {e}")))?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Spawns a publish and returns immediately. Logs on failure.
    pub fn notify(&self, req: EmojiScopeMatchRequest) {
        let client = self.client.clone();
        tokio::spawn(async move {
            let payload = match serde_json::to_vec(&req) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to encode emoji-match request");
                    return;
                }
            };
            if let Err(e) = client
                .publish(SUBJECT_EMOJI_SCOPE_MATCH, payload.into())
                .await
            {
                tracing::warn!(error = %e, "failed to publish emoji-match request");
            }
        });
    }
}
