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
