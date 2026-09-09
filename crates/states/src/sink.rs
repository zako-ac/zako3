//! Where audio should be delivered, and how `ae_proxy` finds it.
//!
//! Two related pieces of state:
//!
//! * **Sink advertisements** — each audio engine and the cache worker publish
//!   the address they receive UDP on, refreshed on a heartbeat. HQ reads this
//!   to fill `deliver_to`.
//! * **Proxy routes** — while the shared proxy owns the only public IP, it needs
//!   `request_id → internal address`. HQ writes that entry *before* telling a
//!   tap to start sending, because a datagram arriving at the proxy for an
//!   unknown request has nowhere to go and is simply dropped.
//!
//! Both disappear once each sink has its own public IP: HQ advertises the
//! sink's own address and stops writing routes, and the proxy can be deleted
//! without touching the protocol.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cache_repo::CacheRepositoryRef;
use crate::error::{Result, StateServiceError};

/// What sort of process is receiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SinkKind {
    /// Plays the audio, and tees a copy to the cache.
    AudioEngine,
    /// Fills the cache with no playback at all.
    Cache,
}

/// One sink's advertised delivery addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkAdvertisement {
    pub sink_id: String,
    pub kind: SinkKind,
    /// Reachable from inside the cluster. What the proxy forwards to.
    pub internal_addr: String,
    /// Reachable from the internet. Unset while the shared proxy owns the only
    /// public IP; once set, taps are pointed straight here and the proxy drops
    /// out of that path.
    #[serde(default)]
    pub public_addr: Option<String>,
}

/// How long a route outlives its last use.
///
/// Deliberately far longer than any request timeout: a route must survive the
/// whole *transfer*, and a long track streams for minutes after the dispatch
/// that created it has returned. Confusing the two would cut long tracks off
/// partway through — a bug that would only ever show up in production, on the
/// longest content.
pub const ROUTE_TTL_SECS: u64 = 3600;

#[derive(Clone)]
pub struct SinkRegistry {
    repo: CacheRepositoryRef,
    lease_ttl_secs: u64,
}

impl SinkRegistry {
    pub fn new(repo: CacheRepositoryRef) -> Self {
        Self { repo, lease_ttl_secs: 45 }
    }

    pub fn with_lease_ttl_secs(mut self, secs: u64) -> Self {
        self.lease_ttl_secs = secs.max(1);
        self
    }

    pub fn lease_ttl_secs(&self) -> u64 {
        self.lease_ttl_secs
    }

    fn sink_key(sink_id: &str) -> String {
        format!("udp:sink:{sink_id}")
    }

    fn route_key(request_id: &Uuid) -> String {
        format!("aeproxy:route:{request_id}")
    }

    /// Advertise a sink, refreshing its lease. Called on a heartbeat.
    pub async fn advertise(&self, ad: &SinkAdvertisement) -> Result<()> {
        let json = serde_json::to_string(ad).map_err(|_| StateServiceError::CacheError)?;
        self.repo
            .set_ex(&Self::sink_key(&ad.sink_id), &json, self.lease_ttl_secs)
            .await;
        Ok(())
    }

    pub async fn get(&self, sink_id: &str) -> Option<SinkAdvertisement> {
        let json = self.repo.get(&Self::sink_key(sink_id)).await?;
        serde_json::from_str(&json).ok()
    }

    /// Addresses to hand a tap, best first.
    ///
    /// A list rather than one address so the proxy can be removed without a
    /// schema change: during the transition it carries the proxy alone, and at
    /// cutover the sink's own address goes first with the proxy behind it as a
    /// fallback.
    pub async fn deliver_to(&self, sink_id: &str, proxy_addr: Option<&str>) -> Vec<String> {
        let ad = self.get(sink_id).await;
        let mut out = Vec::new();

        if let Some(ad) = &ad
            && let Some(public) = &ad.public_addr
        {
            out.push(public.clone());
        }
        if let Some(proxy) = proxy_addr {
            out.push(proxy.to_string());
        }
        out
    }

    /// Point `ae_proxy` at the sink for one request.
    ///
    /// Must be written before the tap is told to send. The proxy caches this in
    /// memory after the first lookup, so the read cost is per request, not per
    /// datagram.
    pub async fn publish_route(&self, request_id: &Uuid, internal_addr: &str) -> Result<()> {
        self.repo
            .set_ex(&Self::route_key(request_id), internal_addr, ROUTE_TTL_SECS)
            .await;
        Ok(())
    }

    pub async fn lookup_route(&self, request_id: &Uuid) -> Option<String> {
        self.repo.get(&Self::route_key(request_id)).await
    }

    /// Drop a route once its transfer is over, rather than waiting out the TTL.
    pub async fn clear_route(&self, request_id: &Uuid) {
        self.repo.del(&Self::route_key(request_id)).await;
    }
}
