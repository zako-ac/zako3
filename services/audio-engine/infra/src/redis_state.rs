use async_trait::async_trait;
use dashmap::DashMap;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::state::StateService,
    types::{ChannelId, GuildId, SessionState},
};

/// Redis-backed session store. Sessions are persisted so that a restarted AE can rejoin its
/// previously-active voice channels (see the rejoin-on-startup path in the controller).
///
/// An in-memory mirror is the authoritative runtime view (fast, infallible); Redis writes are
/// **best-effort** — if Redis is unavailable the AE keeps working, it just won't survive a
/// restart. Keys are namespaced by a hash of the bot token, because a session belongs to a
/// bot identity, not a pod (TL can reassign tokens to pods across restarts).
pub struct RedisStateService {
    conn: Option<redis::aio::MultiplexedConnection>,
    namespace: String,
    sessions: DashMap<(GuildId, ChannelId), SessionState>,
}

impl RedisStateService {
    /// Connect to Redis and load any persisted sessions for `token_namespace` into the
    /// in-memory mirror. Never fails: on any Redis error it logs and starts empty (degraded,
    /// no cross-restart persistence).
    pub async fn new(redis_url: &str, token_namespace: &str) -> Self {
        let namespace = Self::hash_namespace(token_namespace);
        let sessions = DashMap::new();

        let conn = match redis::Client::open(redis_url) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    match Self::load_all(&mut conn, &namespace).await {
                        Ok(loaded) => {
                            for s in loaded {
                                sessions.insert((s.guild_id, s.channel_id), s);
                            }
                            tracing::info!(
                                count = sessions.len(),
                                "RedisStateService: loaded persisted sessions"
                            );
                        }
                        Err(e) => {
                            tracing::warn!("RedisStateService: failed to load sessions: {e}")
                        }
                    }
                    Some(conn)
                }
                Err(e) => {
                    tracing::warn!("RedisStateService: connect failed, running without persistence: {e}");
                    None
                }
            },
            Err(e) => {
                tracing::warn!("RedisStateService: invalid redis url, running without persistence: {e}");
                None
            }
        };

        Self {
            conn,
            namespace,
            sessions,
        }
    }

    fn hash_namespace(token: &str) -> String {
        // SHA-256 of the bot token → first 16 hex chars (64-bit worth).
        // Cryptographic hash ensures determinism across process restarts (unlike
        // DefaultHasher which is randomly seeded per-execution). The token is sensitive,
        // so a cryptographic hash is appropriate even though we only need a stable ID.
        let hash = Sha256::digest(token.as_bytes());
        format!("{:016x}", u64::from_be_bytes(hash[..8].try_into().unwrap()))
    }

    fn key(&self, guild_id: GuildId, channel_id: ChannelId) -> String {
        let g: u64 = guild_id.into();
        let c: u64 = channel_id.into();
        format!("ae:{}:session:{}:{}", self.namespace, g, c)
    }

    async fn load_all(
        conn: &mut redis::aio::MultiplexedConnection,
        namespace: &str,
    ) -> redis::RedisResult<Vec<SessionState>> {
        // Small per-namespace keyspace (a handful of sessions per bot), read only at startup,
        // so KEYS is acceptable here.
        let pattern = format!("ae:{}:session:*", namespace);
        let keys: Vec<String> = conn.keys(&pattern).await?;
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            let val: Option<String> = conn.get(&k).await?;
            if let Some(v) = val {
                match serde_json::from_str::<SessionState>(&v) {
                    Ok(s) => out.push(s),
                    Err(e) => tracing::warn!(key = %k, "RedisStateService: bad session json: {e}"),
                }
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl StateService for RedisStateService {
    async fn get_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>> {
        Ok(self
            .sessions
            .get(&(guild_id, channel_id))
            .map(|s| s.value().clone()))
    }

    async fn save_session(&self, session: &SessionState) -> ZakoResult<()> {
        self.sessions
            .insert((session.guild_id, session.channel_id), session.clone());

        if let Some(mut conn) = self.conn.clone() {
            let key = self.key(session.guild_id, session.channel_id);
            match serde_json::to_string(session) {
                Ok(json) => {
                    let res: redis::RedisResult<()> = conn.set(&key, json).await;
                    if let Err(e) = res {
                        tracing::warn!(%key, "RedisStateService: SET failed (best-effort): {e}");
                    }
                }
                Err(e) => tracing::warn!("RedisStateService: serialize failed: {e}"),
            }
        }
        Ok(())
    }

    async fn delete_session(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        self.sessions.remove(&(guild_id, channel_id));

        if let Some(mut conn) = self.conn.clone() {
            let key = self.key(guild_id, channel_id);
            let res: redis::RedisResult<()> = conn.del(&key).await;
            if let Err(e) = res {
                tracing::warn!(%key, "RedisStateService: DEL failed (best-effort): {e}");
            }
        }
        Ok(())
    }

    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        Ok(self.sessions.iter().map(|s| s.value().clone()).collect())
    }

    async fn list_sessions_in_guild(&self, guild_id: GuildId) -> ZakoResult<Vec<SessionState>> {
        Ok(self
            .sessions
            .iter()
            .filter(|s| s.key().0 == guild_id)
            .map(|s| s.value().clone())
            .collect())
    }
}
