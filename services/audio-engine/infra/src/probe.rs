//! Reads taphub's side of a stalled stream's hop record.
//!
//! taphub records its two hops — chunk arrived from the tap or the cache, chunk
//! handed to the transport — into a short-TTL Redis key per stream. When the
//! engine's jitter buffer gives up waiting, this reader fetches that record so
//! the stall line can carry all four hops on one clock instead of only the two
//! the engine can see.
//!
//! Failure is never fatal and never invents data: no Redis, no record, or a
//! record that is itself stale (the stream ended, the TTL expired) all come back
//! as `None`, and the report says the upstream hops were unavailable.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use zako3_audio_probe::{UpstreamHopSource, UpstreamHops, hop_record_key, now_unix_ms};

/// A record older than this is not evidence about a stream that is stalling now.
///
/// The writer republishes every 200 ms while frames flow, so anything this old
/// means the writer stopped — the stream ended, or taphub died — and reporting
/// its timestamps as "still sending" would be wrong.
const MAX_RECORD_AGE_MS: u64 = 10_000;

pub struct RedisUpstreamHops {
    client: redis::Client,
    conn: Mutex<Option<redis::aio::MultiplexedConnection>>,
}

impl RedisUpstreamHops {
    /// Never fails: a bad URL simply leaves every read returning `None`.
    pub fn new(redis_url: &str) -> Arc<Self> {
        match redis::Client::open(redis_url) {
            Ok(client) => Arc::new(Self {
                client,
                conn: Mutex::new(None),
            }),
            Err(e) => {
                tracing::warn!(%e, "hop record reader: invalid redis url; upstream hops will be unavailable");
                // A client that cannot connect still satisfies the type: keep
                // the same URL so every read fails the same way.
                Arc::new(Self {
                    client: redis::Client::open("redis://127.0.0.1:6379").expect("literal url"),
                    conn: Mutex::new(None),
                })
            }
        }
    }

    async fn connection(&self) -> Option<redis::aio::MultiplexedConnection> {
        let mut guard = self.conn.lock().await;
        if let Some(conn) = guard.as_ref() {
            return Some(conn.clone());
        }
        match tokio::time::timeout(
            Duration::from_millis(500),
            self.client.get_multiplexed_async_connection(),
        )
        .await
        {
            Ok(Ok(conn)) => {
                *guard = Some(conn.clone());
                Some(conn)
            }
            Ok(Err(e)) => {
                tracing::debug!(%e, "hop record reader: redis connect failed");
                None
            }
            Err(_) => {
                tracing::debug!("hop record reader: redis connect timed out");
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl UpstreamHopSource for RedisUpstreamHops {
    async fn read(&self, stream: &str) -> Option<UpstreamHops> {
        use redis::AsyncCommands;

        let Some(mut conn) = self.connection().await else {
            return None;
        };

        let key = hop_record_key(stream);
        let value: Option<String> = match tokio::time::timeout(
            Duration::from_millis(300),
            conn.get::<_, Option<String>>(&key),
        )
        .await
        {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => {
                tracing::debug!(%e, stream, "hop record reader: GET failed");
                return None;
            }
            Err(_) => {
                tracing::debug!(stream, "hop record reader: GET timed out");
                return None;
            }
        };

        let record: UpstreamHops = match value.as_deref().map(serde_json::from_str) {
            Some(Ok(record)) => record,
            Some(Err(e)) => {
                tracing::debug!(%e, stream, "hop record reader: unparseable record");
                return None;
            }
            None => return None,
        };

        if now_unix_ms().saturating_sub(record.updated_unix_ms) > MAX_RECORD_AGE_MS {
            tracing::debug!(
                stream,
                age_ms = now_unix_ms().saturating_sub(record.updated_unix_ms),
                "hop record reader: record is stale; treating upstream as unavailable"
            );
            return None;
        }

        Some(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_missing_record_is_none_not_an_error() {
        // No Redis in this test: every read must simply come back empty.
        let reader = RedisUpstreamHops::new("redis://127.0.0.1:1");
        assert!(reader.read("no-such-stream").await.is_none());
    }
}
