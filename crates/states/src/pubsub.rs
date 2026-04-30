use futures_util::StreamExt;
use redis::AsyncCommands;
use zako3_types::hq::history::UseHistoryEntry;

use crate::error::{Result, StateServiceError};

pub const HISTORY_CHANNEL: &str = "history";

#[derive(Clone)]
pub struct RedisPubSub {
    client: redis::Client,
    conn_mgr: redis::aio::ConnectionManager,
}

impl RedisPubSub {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn_mgr = client.get_connection_manager().await?;
        Ok(Self { client, conn_mgr })
    }

    pub async fn publish_history(&self, entry: &UseHistoryEntry) -> Result<()> {
        let payload = serde_json::to_string(entry).map_err(|_| StateServiceError::CacheError)?;
        let mut conn = self.conn_mgr.clone();
        let _: () = conn.publish(HISTORY_CHANNEL, payload).await?;
        Ok(())
    }

    /// Subscribes to the history channel and returns an async stream of entries.
    /// Invalid messages are silently skipped.
    pub async fn subscribe_history(
        self,
    ) -> Result<impl futures_util::Stream<Item = UseHistoryEntry>> {
        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub.subscribe(HISTORY_CHANNEL).await?;
        let stream = pubsub.into_on_message().filter_map(|msg| async move {
            let payload: String = msg.get_payload().ok()?;
            serde_json::from_str::<UseHistoryEntry>(&payload).ok()
        });
        Ok(stream)
    }
}
