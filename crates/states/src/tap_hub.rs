use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// How a tap's last probe went, as a single word for operators and routers.
///
/// `Unknown` is deliberately not a failure. Every tap starts unknown the day
/// this ships, and an unprobed tap has to keep being routed to exactly as it
/// was before there was a health store at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapVerdict {
    /// Nothing has probed it, or the probe said nothing about synthesis.
    Unknown,
    /// A probe answered, or the tap only has connections we have no reason to
    /// doubt.
    Healthy,
    /// A first sample was late. Deprioritised, not excluded.
    Busy,
    /// Every connection we know about failed its probe.
    Unhealthy,
}

impl TapVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            TapVerdict::Unknown => "unknown",
            TapVerdict::Healthy => "healthy",
            TapVerdict::Busy => "busy",
            TapVerdict::Unhealthy => "unhealthy",
        }
    }
}

/// What one probe concluded about one connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionVerdict {
    /// Answered the probe inside the first-sample deadline.
    Healthy,
    /// Answered, but took too long to produce a sample. Deprioritised rather
    /// than excluded: slow is not the same as broken, and this tap is still the
    /// only one that can serve its own id.
    Slow,
    /// Did not answer, or answered that it could not synthesize.
    Failed,
    /// The tap does not implement the probe, so it says nothing either way.
    /// Treated as usable, because inventing a verdict for a tap that has told
    /// us nothing is how a working tap gets taken out of service.
    Unsupported,
}

/// One connection's probe result, as seen by one replica.
///
/// `connection_id` is unique only within the replica that allocated it, which
/// is why every record is keyed by `(replica_id, connection_id)` and never by
/// the id alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionHealth {
    pub connection_id: u64,
    pub verdict: ConnectionVerdict,
    /// What the probe measured, when it got far enough to measure anything.
    pub time_to_first_sample_ms: Option<u64>,
    /// Why it failed, verbatim, for a human reading `/taps/:id/stats`.
    pub reason: Option<String>,
    pub probed_at: DateTime<Utc>,
}

/// One replica's word on one tap.
///
/// Per-replica rather than per-tap for the same reason presence is: the probe
/// runs wherever the connection lives, and two replicas writing one shared key
/// would clobber each other's evidence. Readers union the live records
/// instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapHealth {
    pub replica_id: String,
    pub connections: Vec<ConnectionHealth>,
    pub consecutive_failures: u32,
    pub last_probe_at: Option<DateTime<Utc>>,
    /// The last probe's outcome in words — an operator's answer to "why".
    pub last_probe_result: Option<String>,
    /// The most recent arm-to-first-sample interval this replica measured, in
    /// milliseconds.
    pub time_to_first_sample_ms: Option<u64>,
    pub updated_at: DateTime<Utc>,
}

impl TapHealth {
    /// Whether nothing is known about the tap yet, or rather nothing usable.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

/// What real requests, rather than probes, have been saying about a tap's
/// first sample.
///
/// Tap-global and not per-replica: the audio engine measured it and the audio
/// engine does not hold connections, so there is no replica to attribute it to.
/// The consequence is deliberate — one slow request deprioritises the whole tap
/// — because HQ cannot tell which connection produced the slow sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapLatency {
    /// Exponential moving average of the arm-to-first-frame interval, in
    /// milliseconds. Smoothed because one sample of one track is noise, and a
    /// verdict that flips on noise is worse than no verdict.
    pub time_to_first_sample_ms: Option<u64>,
    /// Consecutive requests whose first sample was late or never arrived.
    pub consecutive_slow: u32,
    /// While this is in the future the tap's connections are deprioritised.
    pub busy_until: Option<DateTime<Utc>>,
    pub reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl TapLatency {
    fn is_busy_at(&self, now: DateTime<Utc>) -> bool {
        self.busy_until.is_some_and(|until| until > now)
    }
}

/// Default lease for a health record. Longer than a probe interval by enough
/// that two missed probes do not make a healthy replica look gone, and short
/// enough that a crashed replica's last word stops being trusted on its own.
pub const DEFAULT_HEALTH_TTL_SECS: u64 = 90;

/// How long a first sample may take before it counts against a tap.
///
/// Two seconds is comfortably above a tap synthesizing a short phrase from a
/// cold cache and comfortably below what a listener would call broken.
pub const DEFAULT_SLOW_FIRST_SAMPLE_MS: u64 = 2_000;

/// Default lease for a busy verdict. Short on purpose: a tap that was slow once
/// should be tried again soon rather than written off for a whole minute.
pub const DEFAULT_BUSY_TTL_SECS: u64 = 30;

/// How many consecutive late first samples make a tap busy. A single slow start
/// is a cold cache, not a broken tap.
pub const BUSY_AFTER_SLOW_SAMPLES: u32 = 3;

/// Per-tap health, written by whichever replica probed and read by every
/// replica that routes.
///
/// Same shape as [`GatewayPresenceService`] — a per-replica key carrying the
/// lease, plus a set index that readers prune — with one addition: a single
/// tap-global `tap:latency:{tap_id}` lease for what real requests observed.
///
/// ```text
/// SADD  tap:health:{tap_id}              {replica_id}
/// SETEX tap:health:{tap_id}:{replica_id} <ttl> <json>
/// SETEX tap:latency:{tap_id}             <ttl> <json>
/// ```
#[derive(Clone)]
pub struct TapHealthService {
    cache_repository: CacheRepositoryRef,
    lease_ttl_secs: u64,
    busy_ttl_secs: u64,
    /// A first sample slower than this counts against the tap.
    slow_first_sample_ms: u64,
}

impl TapHealthService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self {
            cache_repository,
            lease_ttl_secs: DEFAULT_HEALTH_TTL_SECS,
            busy_ttl_secs: DEFAULT_BUSY_TTL_SECS,
            slow_first_sample_ms: DEFAULT_SLOW_FIRST_SAMPLE_MS,
        }
    }

    pub fn with_lease_ttl_secs(mut self, secs: u64) -> Self {
        self.lease_ttl_secs = secs.max(1);
        self
    }

    pub fn with_busy_ttl_secs(mut self, secs: u64) -> Self {
        self.busy_ttl_secs = secs.max(1);
        self
    }

    /// At what first-sample latency a tap starts being counted as slow.
    pub fn with_slow_first_sample_ms(mut self, ms: u64) -> Self {
        self.slow_first_sample_ms = ms.max(1);
        self
    }

    pub fn lease_ttl_secs(&self) -> u64 {
        self.lease_ttl_secs
    }

    pub fn busy_ttl_secs(&self) -> u64 {
        self.busy_ttl_secs
    }

    pub fn slow_first_sample_ms(&self) -> u64 {
        self.slow_first_sample_ms
    }

    fn index_key(tap_id: &TapId) -> String {
        format!("tap:health:{}", tap_id.0)
    }

    fn replica_key(tap_id: &TapId, replica_id: &str) -> String {
        format!("tap:health:{}:{}", tap_id.0, replica_id)
    }

    fn latency_key(tap_id: &TapId) -> String {
        format!("tap:latency:{}", tap_id.0)
    }

    /// Publish this replica's probe results for a tap, replacing whatever it
    /// said last time.
    ///
    /// A whole-list write, never a delta: this replica is the only writer of its
    /// own key, so there is nothing to read-modify-write against. An empty list
    /// withdraws the record rather than publishing "no connections, all well" —
    /// a replica that holds nothing for a tap has no business having an opinion
    /// about it.
    pub async fn publish_probe(
        &self,
        tap_id: &TapId,
        replica_id: &str,
        connections: &[ConnectionHealth],
    ) -> Result<()> {
        if connections.is_empty() {
            return self.withdraw(tap_id, replica_id).await;
        }

        let previous = self.replica_record(tap_id, replica_id).await;
        let consecutive_failures = if connections
            .iter()
            .any(|c| c.verdict == ConnectionVerdict::Failed)
        {
            previous.map(|p| p.consecutive_failures).unwrap_or(0) + 1
        } else {
            0
        };

        let time_to_first_sample_ms = connections
            .iter()
            .filter_map(|c| c.time_to_first_sample_ms)
            .min();

        let record = TapHealth {
            replica_id: replica_id.to_string(),
            connections: connections.to_vec(),
            consecutive_failures,
            last_probe_at: Some(Utc::now()),
            last_probe_result: Some(summarize_probe(connections)),
            time_to_first_sample_ms,
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&record).map_err(|_| StateServiceError::CacheError)?;
        let _ = self
            .cache_repository
            .sadd(&Self::index_key(tap_id), replica_id)
            .await;
        self.cache_repository
            .set_ex(
                &Self::replica_key(tap_id, replica_id),
                &json,
                self.lease_ttl_secs,
            )
            .await;
        Ok(())
    }

    /// Drop this replica's whole opinion about a tap.
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

    /// Join a probe round that ended with nothing to probe: keep the failure
    /// count moving so a tap whose every connection refuses stays unhealthy
    /// instead of silently reverting to unknown.
    pub async fn record_probe_failure(&self, tap_id: &TapId, replica_id: &str, reason: &str) -> Result<()> {
        let previous = self.replica_record(tap_id, replica_id).await;
        let record = TapHealth {
            replica_id: replica_id.to_string(),
            connections: previous
                .as_ref()
                .map(|p| p.connections.clone())
                .unwrap_or_default(),
            consecutive_failures: previous
                .as_ref()
                .map(|p| p.consecutive_failures)
                .unwrap_or(0)
                + 1,
            last_probe_at: Some(Utc::now()),
            last_probe_result: Some(reason.to_string()),
            time_to_first_sample_ms: previous.and_then(|p| p.time_to_first_sample_ms),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&record).map_err(|_| StateServiceError::CacheError)?;
        let _ = self
            .cache_repository
            .sadd(&Self::index_key(tap_id), replica_id)
            .await;
        self.cache_repository
            .set_ex(
                &Self::replica_key(tap_id, replica_id),
                &json,
                self.lease_ttl_secs,
            )
            .await;
        Ok(())
    }

    /// Note how long a real request's first sample took, and degrade the tap if
    /// that keeps happening.
    ///
    /// Returns whether the tap is now busy, so the caller can log the decision
    /// rather than infer it.
    pub async fn record_first_sample(
        &self,
        tap_id: &TapId,
        observed_ms: Option<u64>,
    ) -> Result<bool> {
        let previous = self
            .cache_repository
            .get(&Self::latency_key(tap_id))
            .await
            .and_then(|json| serde_json::from_str::<TapLatency>(&json).ok());

        let late = match observed_ms {
            Some(ms) => ms > self.slow_first_sample_ms,
            // No frame ever arrived, so there is nothing to measure.
            None => true,
        };

        let consecutive_slow = if late {
            previous.as_ref().map(|p| p.consecutive_slow).unwrap_or(0) + 1
        } else {
            0
        };

        // A timeout is a failed request for the listener, so it counts at once.
        // A merely late sample has to happen repeatedly before it is an opinion.
        let busy = observed_ms.is_none() || consecutive_slow >= BUSY_AFTER_SLOW_SAMPLES;

        let now = Utc::now();
        let ewma = match (previous.as_ref().and_then(|p| p.time_to_first_sample_ms), observed_ms)
        {
            (Some(prev), Some(ms)) => Some((prev as f64 * 0.7 + ms as f64 * 0.3).round() as u64),
            (None, Some(ms)) => Some(ms),
            // Keep the last real measurement across a miss: the miss is already
            // represented by the failure count.
            (prev, None) => prev,
        };

        let record = TapLatency {
            time_to_first_sample_ms: ewma,
            consecutive_slow,
            busy_until: busy.then(|| now + chrono::Duration::seconds(self.busy_ttl_secs as i64)),
            reason: busy.then(|| match observed_ms {
                Some(ms) => format!(
                    "first sample took {ms}ms, over the {}ms threshold, {consecutive_slow} in a row",
                    self.slow_first_sample_ms
                ),
                None => format!("no first sample ever arrived, {consecutive_slow} in a row"),
            }),
            updated_at: now,
        };

        let json = serde_json::to_string(&record).map_err(|_| StateServiceError::CacheError)?;
        // Leased well past the busy window so a stray read cannot resurrect a
        // stale verdict, and so a replica that stops reporting stops counting.
        let ttl = self.busy_ttl_secs * 4;
        self.cache_repository
            .set_ex(&Self::latency_key(tap_id), &json, ttl)
            .await;
        Ok(busy)
    }

    /// Mark a tap busy for the configured window without claiming to have
    /// measured anything. Used when a dispatch failed in a way that says the
    /// tap, not the request, was the problem.
    pub async fn mark_busy(&self, tap_id: &TapId, reason: &str) -> Result<()> {
        let previous = self
            .cache_repository
            .get(&Self::latency_key(tap_id))
            .await
            .and_then(|json| serde_json::from_str::<TapLatency>(&json).ok());
        let now = Utc::now();
        let record = TapLatency {
            time_to_first_sample_ms: previous
                .as_ref()
                .and_then(|p| p.time_to_first_sample_ms),
            consecutive_slow: previous.as_ref().map(|p| p.consecutive_slow).unwrap_or(0) + 1,
            busy_until: Some(now + chrono::Duration::seconds(self.busy_ttl_secs as i64)),
            reason: Some(reason.to_string()),
            updated_at: now,
        };
        let json = serde_json::to_string(&record).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository
            .set_ex(
                &Self::latency_key(tap_id),
                &json,
                self.busy_ttl_secs * 4,
            )
            .await;
        Ok(())
    }

    /// Every live replica's word on this tap, plus what real requests observed.
    ///
    /// Prunes index members whose lease has lapsed, exactly as presence does: a
    /// replica that died cannot retract its verdict, so the lease has to.
    pub async fn get(&self, tap_id: &TapId) -> Result<TapHealthView> {
        let index = Self::index_key(tap_id);
        let replicas = self.cache_repository.smembers(&index).await.unwrap_or_default();

        let mut records = Vec::new();
        for replica_id in replicas {
            let key = Self::replica_key(tap_id, &replica_id);
            match self.cache_repository.get(&key).await {
                Some(json) => match serde_json::from_str::<TapHealth>(&json) {
                    Ok(record) => records.push(record),
                    Err(_) => {
                        tracing::warn!(replica_id, "unparseable tap health; pruning");
                        let _ = self.cache_repository.srem(&index, &replica_id).await;
                    }
                },
                None => {
                    let _ = self.cache_repository.srem(&index, &replica_id).await;
                }
            }
        }

        let latency = self
            .cache_repository
            .get(&Self::latency_key(tap_id))
            .await
            .and_then(|json| serde_json::from_str::<TapLatency>(&json).ok());

        Ok(TapHealthView { records, latency })
    }

    async fn replica_record(&self, tap_id: &TapId, replica_id: &str) -> Option<TapHealth> {
        self.cache_repository
            .get(&Self::replica_key(tap_id, replica_id))
            .await
            .and_then(|json| serde_json::from_str::<TapHealth>(&json).ok())
    }
}

/// A tap's health, merged across replicas into the two questions a router
/// actually asks.
#[derive(Debug, Clone, Default)]
pub struct TapHealthView {
    records: Vec<TapHealth>,
    latency: Option<TapLatency>,
}

impl TapHealthView {
    /// Build a view from records that did not come out of Redis.
    ///
    /// The routing decision is the part worth testing, and it should not need a
    /// server to be exercised.
    pub fn from_records(records: Vec<TapHealth>, latency: Option<TapLatency>) -> Self {
        Self { records, latency }
    }

    pub fn records(&self) -> &[TapHealth] {
        &self.records
    }

    /// Every connection any live replica has judged, as `(replica, connection)`.
    pub fn connections(&self) -> impl Iterator<Item = (&str, &ConnectionHealth)> {
        self.records.iter().flat_map(|r| {
            r.connections
                .iter()
                .map(move |c| (r.replica_id.as_str(), c))
        })
    }

    pub fn busy_until(&self) -> Option<DateTime<Utc>> {
        self.latency
            .as_ref()
            .filter(|l| l.is_busy_at(Utc::now()))
            .and_then(|l| l.busy_until)
    }

    pub fn is_busy(&self) -> bool {
        self.busy_until().is_some()
    }

    pub fn busy_reason(&self) -> Option<&str> {
        self.latency
            .as_ref()
            .filter(|l| l.is_busy_at(Utc::now()))
            .and_then(|l| l.reason.as_deref())
    }

    /// The most recent arm-to-first-sample interval anyone measured.
    ///
    /// A real request beats a probe: it is what a listener waited for.
    pub fn time_to_first_sample_ms(&self) -> Option<u64> {
        self.latency
            .as_ref()
            .and_then(|l| l.time_to_first_sample_ms)
            .or_else(|| {
                self.records
                    .iter()
                    .filter_map(|r| r.time_to_first_sample_ms)
                    .min()
            })
    }

    /// Consecutive probe failures, worst across replicas.
    pub fn consecutive_failures(&self) -> u32 {
        self.records
            .iter()
            .map(|r| r.consecutive_failures)
            .max()
            .unwrap_or(0)
    }

    pub fn last_probe_at(&self) -> Option<DateTime<Utc>> {
        self.records.iter().filter_map(|r| r.last_probe_at).max()
    }

    pub fn last_probe_result(&self) -> Option<&str> {
        self.records
            .iter()
            .max_by_key(|r| r.last_probe_at)
            .and_then(|r| r.last_probe_result.as_deref())
    }

    /// The one-word answer, for operators and for `get_tap_stats`.
    pub fn verdict(&self) -> TapVerdict {
        if self.is_busy() {
            return TapVerdict::Busy;
        }

        let mut probed = 0usize;
        let mut usable = 0usize;
        for (_, c) in self.connections() {
            probed += 1;
            if c.verdict != ConnectionVerdict::Failed {
                usable += 1;
            }
        }

        match (probed, usable) {
            (0, _) => TapVerdict::Unknown,
            (_, 0) => TapVerdict::Unhealthy,
            _ => TapVerdict::Healthy,
        }
    }

    /// Whether a tap is worth routing to at all.
    ///
    /// Only a tap whose every probed connection has failed is unusable. Busy is
    /// not a reason to refuse — a busy tap is still the only tap that can serve
    /// its own id — and neither is `Unknown`: most taps will be unknown for a
    /// while yet.
    pub fn is_usable(&self) -> bool {
        self.verdict() != TapVerdict::Unhealthy
    }

    /// Connections a router must not pick.
    ///
    /// Keyed by replica as well as connection because `connection_id` is
    /// allocated per process — replica A's connection 3 and replica B's
    /// connection 3 are different connections.
    pub fn is_excluded(&self, replica_id: &str, connection_id: u64) -> bool {
        self.records.iter().any(|r| {
            r.replica_id == replica_id
                && r.connections
                    .iter()
                    .any(|c| c.connection_id == connection_id && c.verdict == ConnectionVerdict::Failed)
        })
    }

    /// Everything a router is not to pick, as `(replica, connection)`.
    pub fn excluded_connections(&self) -> Vec<(String, u64)> {
        self.records
            .iter()
            .flat_map(|r| {
                r.connections
                    .iter()
                    .filter(|c| c.verdict == ConnectionVerdict::Failed)
                    .map(|c| (r.replica_id.clone(), c.connection_id))
            })
            .collect()
    }

    /// Weight scale for a connection: `0.0` deprioritises without excluding.
    ///
    /// Busy zeroes the whole tap because HQ cannot tell which connection was
    /// slow — the audio engine measured the stream, not the socket. Slow probes
    /// zero one connection. Neither is an exclusion: see [`Self::is_usable`].
    pub fn weight_scale(&self, replica_id: &str, connection_id: u64) -> f32 {
        if self.is_busy() {
            return 0.0;
        }
        let slow = self.records.iter().any(|r| {
            r.replica_id == replica_id
                && r.connections.iter().any(|c| {
                    c.connection_id == connection_id && c.verdict == ConnectionVerdict::Slow
                })
        });
        if slow { 0.0 } else { 1.0 }
    }
}

/// A one-line summary of a probe round, for the record and for operators.
fn summarize_probe(connections: &[ConnectionHealth]) -> String {
    let failed = connections
        .iter()
        .filter(|c| c.verdict == ConnectionVerdict::Failed)
        .count();
    let slow = connections
        .iter()
        .filter(|c| c.verdict == ConnectionVerdict::Slow)
        .count();
    let unsupported = connections
        .iter()
        .filter(|c| c.verdict == ConnectionVerdict::Unsupported)
        .count();
    let best = connections
        .iter()
        .filter_map(|c| c.time_to_first_sample_ms)
        .min();

    let mut parts = vec![format!("{} connection(s)", connections.len())];
    parts.push(match best {
        Some(ms) => format!("fastest synthesis {ms}ms"),
        None => "nothing measured".to_string(),
    });
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    if slow > 0 {
        parts.push(format!("{slow} over the deadline"));
    }
    if unsupported > 0 {
        parts.push(format!("{unsupported} without probe support"));
    }
    parts.join(", ")
}
