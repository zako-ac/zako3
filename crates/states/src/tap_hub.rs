use zako3_types::{OnlineTapStates, TapName, hq::TapId};

use crate::cache_repo::CacheRepositoryRef;
use crate::error::{Result, StateServiceError};

/// Default lease TTL used when none is configured. The taphub overrides this via
/// [`TapHubStateService::with_lease_ttl_secs`]; read-only consumers (hq, workers)
/// never publish so the value is irrelevant to them.
pub const DEFAULT_LEASE_TTL_SECS: u64 = 30;

#[derive(Clone)]
pub struct TapHubStateService {
    pub cache_repository: CacheRepositoryRef,
    /// TTL applied to every published connection-state key. The taphub refreshes
    /// these keys on a heartbeat well within the TTL; if the process dies the
    /// keys expire on their own, so stale "online" state can never linger.
    lease_ttl_secs: u64,
}

impl TapHubStateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self {
            cache_repository,
            lease_ttl_secs: DEFAULT_LEASE_TTL_SECS,
        }
    }

    /// Set the TTL (seconds) for published connection-state leases.
    pub fn with_lease_ttl_secs(mut self, secs: u64) -> Self {
        self.lease_ttl_secs = secs.max(1);
        self
    }

    /// The configured lease TTL in seconds. Callers derive the heartbeat interval
    /// from this (typically `ttl / 3`).
    pub fn lease_ttl_secs(&self) -> u64 {
        self.lease_ttl_secs
    }

    pub async fn get_tap_id_by_name(&self, tap_name: &TapName) -> Result<Option<TapId>> {
        let name_key = format!("tap_name:{}", tap_name.0);
        let id_str = self.cache_repository.get(&name_key).await;
        Ok(id_str.map(TapId))
    }

    pub async fn get_tap_states(&self, tap_id: &TapId) -> Result<OnlineTapStates> {
        let key = format!("tap:{}", tap_id.0);

        let state_str = match self.cache_repository.get(&key).await {
            Some(state) => state,
            None => return Ok(vec![]),
        };

        let state: OnlineTapStates =
            serde_json::from_str(&state_str).map_err(|_| StateServiceError::CacheError)?;

        Ok(state)
    }

    /// Publish the complete live connection set for a tap, with a TTL lease.
    ///
    /// The taphub registry is the single writer, so we always write the full list
    /// rather than read-modify-write a shared list. An empty list deletes the key
    /// immediately (last connection gone); otherwise the key (and the name index)
    /// are written with `set_ex(lease_ttl_secs)` and must be refreshed before the
    /// TTL elapses to stay online.
    pub async fn publish_tap_states(
        &self,
        tap_id: &TapId,
        states: &OnlineTapStates,
    ) -> Result<()> {
        let key = format!("tap:{}", tap_id.0);

        if states.is_empty() {
            self.cache_repository.del(&key).await;
            return Ok(());
        }

        if let Some(first) = states.first() {
            let name_key = format!("tap_name:{}", first.tap_name.0);
            self.cache_repository
                .set_ex(&name_key, &tap_id.0, self.lease_ttl_secs)
                .await;
        }

        let state_str = serde_json::to_string(states).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository
            .set_ex(&key, &state_str, self.lease_ttl_secs)
            .await;

        Ok(())
    }

    pub async fn get_online_count(&self, tap_id: &TapId) -> Result<usize> {
        Ok(self.get_tap_states(tap_id).await?.len())
    }
}

/// Per-replica presence for the HQ gateway.
///
/// [`TapHubStateService::publish_tap_states`] cannot be reused here. It writes
/// the *whole* connection list under one key, which is correct only because
/// taphub is a single process — with N gateway replicas they would clobber each
/// other, each publishing only the connections it happens to hold.
///
/// So each replica owns its own key and the reader unions them:
///
/// ```text
/// SADD  tap:gw:{tap_id}                {replica_id}
/// SETEX tap:gw:{tap_id}:{replica_id}   <ttl> <json>
/// ```
///
/// Redis has no per-member TTL on a set, so the set is the index and the
/// per-replica keys carry the lease. A replica that dies stops refreshing, its
/// key expires, and the next reader prunes it from the set — no coordination
/// and no cleanup path that has to run on the way down.
#[derive(Clone)]
pub struct GatewayPresenceService {
    cache_repository: CacheRepositoryRef,
    lease_ttl_secs: u64,
}

impl GatewayPresenceService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self {
            cache_repository,
            lease_ttl_secs: DEFAULT_LEASE_TTL_SECS,
        }
    }

    pub fn with_lease_ttl_secs(mut self, secs: u64) -> Self {
        self.lease_ttl_secs = secs.max(1);
        self
    }

    pub fn lease_ttl_secs(&self) -> u64 {
        self.lease_ttl_secs
    }

    fn index_key(tap_id: &TapId) -> String {
        format!("tap:gw:{}", tap_id.0)
    }

    fn replica_key(tap_id: &TapId, replica_id: &str) -> String {
        format!("tap:gw:{}:{}", tap_id.0, replica_id)
    }

    /// Publish this replica's connections for a tap, refreshing its lease.
    ///
    /// Writes the full list *for this replica only*. An empty list drops the
    /// key and leaves the set index, which the next read prunes.
    pub async fn publish(
        &self,
        tap_id: &TapId,
        replica_id: &str,
        states: &OnlineTapStates,
    ) -> Result<()> {
        let key = Self::replica_key(tap_id, replica_id);
        if states.is_empty() {
            self.cache_repository.del(&key).await;
            let _ = self
                .cache_repository
                .srem(&Self::index_key(tap_id), replica_id)
                .await;
            return Ok(());
        }

        let json = serde_json::to_string(states).map_err(|_| StateServiceError::CacheError)?;
        let _ = self
            .cache_repository
            .sadd(&Self::index_key(tap_id), replica_id)
            .await;
        self.cache_repository
            .set_ex(&key, &json, self.lease_ttl_secs)
            .await;

        if let Some(first) = states.first() {
            let name_key = format!("tap_name:{}", first.tap_name.0);
            self.cache_repository
                .set_ex(&name_key, &tap_id.0, self.lease_ttl_secs)
                .await;
        }
        Ok(())
    }

    /// Every live connection for a tap, across all replicas.
    ///
    /// Prunes index members whose lease has lapsed, so a crashed replica stops
    /// being selectable without anything having to clean up after it.
    pub async fn get(&self, tap_id: &TapId) -> Result<OnlineTapStates> {
        let index = Self::index_key(tap_id);
        let replicas = self.cache_repository.smembers(&index).await.unwrap_or_default();

        let mut out = Vec::new();
        for replica_id in replicas {
            let key = Self::replica_key(tap_id, &replica_id);
            match self.cache_repository.get(&key).await {
                Some(json) => match serde_json::from_str::<OnlineTapStates>(&json) {
                    Ok(states) => out.extend(states),
                    Err(_) => {
                        tracing::warn!(replica_id, "unparseable gateway presence; pruning");
                        let _ = self.cache_repository.srem(&index, &replica_id).await;
                    }
                },
                None => {
                    // Lease lapsed: that replica is gone or wedged.
                    let _ = self.cache_repository.srem(&index, &replica_id).await;
                }
            }
        }
        Ok(out)
    }

    /// Drop this replica's entry for a tap. Best effort — the lease is what
    /// actually guarantees the entry goes away.
    pub async fn withdraw(&self, tap_id: &TapId, replica_id: &str) -> Result<()> {
        self.cache_repository
            .del(&Self::replica_key(tap_id, replica_id))
            .await;
        let _ = self
            .cache_repository
            .srem(&Self::index_key(tap_id), replica_id)
            .await;
        Ok(())
    }
}
