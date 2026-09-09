//! Sink advertisements and proxy routes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;
use zako3_states::cache_repo::{CacheRepository, CacheRepositoryRef};
use zako3_states::error::Result;
use zako3_states::sink::ROUTE_TTL_SECS;
use zako3_states::{SinkAdvertisement, SinkKind, SinkRegistry};

/// Records the TTL each key was written with, so "route outlives the request"
/// can actually be asserted rather than assumed.
#[derive(Default)]
struct MemRepo {
    strings: Mutex<HashMap<String, (String, Option<u64>)>>,
    sets: Mutex<HashMap<String, HashSet<String>>>,
}

impl MemRepo {
    fn ttl_of(&self, key: &str) -> Option<u64> {
        self.strings.lock().unwrap().get(key).and_then(|(_, t)| *t)
    }
    fn expire(&self, key: &str) {
        self.strings.lock().unwrap().remove(key);
    }
}

#[async_trait]
impl CacheRepository for MemRepo {
    async fn get(&self, key: &str) -> Option<String> {
        self.strings.lock().unwrap().get(key).map(|(v, _)| v.clone())
    }
    async fn set(&self, key: &str, value: &str) {
        self.strings
            .lock()
            .unwrap()
            .insert(key.into(), (value.into(), None));
    }
    async fn set_ex(&self, key: &str, value: &str, ttl: u64) {
        self.strings
            .lock()
            .unwrap()
            .insert(key.into(), (value.into(), Some(ttl)));
    }
    async fn del(&self, key: &str) {
        self.strings.lock().unwrap().remove(key);
    }
    async fn incr(&self, _k: &str) -> Result<i64> { Ok(0) }
    async fn decr(&self, _k: &str) -> Result<i64> { Ok(0) }
    async fn incrby(&self, _k: &str, _a: i64) -> Result<i64> { Ok(0) }
    async fn pfadd(&self, _k: &str, _e: &str) -> Result<()> { Ok(()) }
    async fn pfcount(&self, _k: &str) -> Result<u64> { Ok(0) }
    async fn pfcount_multi(&self, _k: &[String]) -> Result<u64> { Ok(0) }
    async fn sadd(&self, key: &str, member: &str) -> Result<()> {
        self.sets.lock().unwrap().entry(key.into()).or_default().insert(member.into());
        Ok(())
    }
    async fn srem(&self, key: &str, member: &str) -> Result<()> {
        if let Some(s) = self.sets.lock().unwrap().get_mut(key) {
            s.remove(member);
        }
        Ok(())
    }
    async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        Ok(self.sets.lock().unwrap().get(key).map(|s| s.iter().cloned().collect()).unwrap_or_default())
    }
    async fn hgetall(&self, _k: &str) -> Result<Vec<(String, String)>> { Ok(vec![]) }
    async fn hincrby(&self, _k: &str, _f: &str, _a: i64) -> Result<i64> { Ok(0) }
    async fn hdel_key(&self, _k: &str) -> Result<()> { Ok(()) }
}

fn setup() -> (Arc<MemRepo>, SinkRegistry) {
    let repo = Arc::new(MemRepo::default());
    (repo.clone(), SinkRegistry::new(repo as CacheRepositoryRef))
}

fn ae(public: Option<&str>) -> SinkAdvertisement {
    SinkAdvertisement {
        sink_id: "ae-0".into(),
        kind: SinkKind::AudioEngine,
        internal_addr: "10.0.0.7:5000".into(),
        public_addr: public.map(str::to_string),
    }
}

#[tokio::test]
async fn an_advertisement_round_trips() {
    let (_repo, reg) = setup();
    reg.advertise(&ae(None)).await.unwrap();

    let got = reg.get("ae-0").await.expect("advertised");
    assert_eq!(got.internal_addr, "10.0.0.7:5000");
    assert_eq!(got.kind, SinkKind::AudioEngine);
}

#[tokio::test]
async fn an_unadvertised_sink_is_unknown() {
    let (_repo, reg) = setup();
    assert!(reg.get("nobody").await.is_none());
}

/// A sink that stops heartbeating disappears, so HQ stops handing taps an
/// address nothing is listening on.
#[tokio::test]
async fn an_advertisement_expires_with_its_lease() {
    let (repo, reg) = setup();
    reg.advertise(&ae(None)).await.unwrap();
    assert!(repo.ttl_of("udp:sink:ae-0").is_some(), "must be written with a lease");

    repo.expire("udp:sink:ae-0");
    assert!(reg.get("ae-0").await.is_none());
}

/// While the shared proxy owns the only public IP, that is the address taps get.
#[tokio::test]
async fn during_transit_taps_are_pointed_at_the_proxy() {
    let (_repo, reg) = setup();
    reg.advertise(&ae(None)).await.unwrap();

    let to = reg.deliver_to("ae-0", Some("proxy.zako.ac:5000")).await;
    assert_eq!(to, vec!["proxy.zako.ac:5000".to_string()]);
}

/// Once a sink has its own public IP it goes first, with the proxy behind it.
/// A tap follows that cutover with no code change, which is the whole reason
/// `deliver_to` is a list.
#[tokio::test]
async fn after_cutover_the_sink_comes_first_and_the_proxy_is_a_fallback() {
    let (_repo, reg) = setup();
    reg.advertise(&ae(Some("1.2.3.4:5000"))).await.unwrap();

    let to = reg.deliver_to("ae-0", Some("proxy.zako.ac:5000")).await;
    assert_eq!(
        to,
        vec!["1.2.3.4:5000".to_string(), "proxy.zako.ac:5000".to_string()]
    );
}

#[tokio::test]
async fn with_no_proxy_and_no_public_address_there_is_nowhere_to_send() {
    let (_repo, reg) = setup();
    reg.advertise(&ae(None)).await.unwrap();
    assert!(reg.deliver_to("ae-0", None).await.is_empty());
}

#[tokio::test]
async fn a_route_points_the_proxy_at_the_sink() {
    let (_repo, reg) = setup();
    let id = Uuid::new_v4();

    reg.publish_route(&id, "10.0.0.7:5000").await.unwrap();
    assert_eq!(reg.lookup_route(&id).await, Some("10.0.0.7:5000".into()));
}

/// A route has to outlive the whole *transfer*, not the dispatch that created
/// it. A ten-minute track streams long after its request returned, and a TTL
/// sized to the request timeout would cut it off partway through — a failure
/// that would only ever appear on the longest content.
#[tokio::test]
async fn a_route_outlives_any_request_timeout() {
    let (repo, reg) = setup();
    let id = Uuid::new_v4();
    reg.publish_route(&id, "10.0.0.7:5000").await.unwrap();

    let ttl = repo.ttl_of(&format!("aeproxy:route:{id}")).expect("leased");
    assert_eq!(ttl, ROUTE_TTL_SECS);
    assert!(ttl >= 3600, "a long track must not outlive its own route");
}

#[tokio::test]
async fn clearing_a_route_removes_it_rather_than_waiting_out_the_ttl() {
    let (_repo, reg) = setup();
    let id = Uuid::new_v4();

    reg.publish_route(&id, "10.0.0.7:5000").await.unwrap();
    reg.clear_route(&id).await;
    assert!(reg.lookup_route(&id).await.is_none());
}

#[tokio::test]
async fn routes_for_different_requests_are_independent() {
    let (_repo, reg) = setup();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    reg.publish_route(&a, "10.0.0.1:5000").await.unwrap();
    reg.publish_route(&b, "10.0.0.2:5000").await.unwrap();
    reg.clear_route(&a).await;

    assert!(reg.lookup_route(&a).await.is_none());
    assert_eq!(reg.lookup_route(&b).await, Some("10.0.0.2:5000".into()));
}

#[tokio::test]
async fn a_cache_worker_advertises_the_same_way_an_engine_does() {
    let (_repo, reg) = setup();
    reg.advertise(&SinkAdvertisement {
        sink_id: "cache-0".into(),
        kind: SinkKind::Cache,
        internal_addr: "10.0.0.9:4101".into(),
        public_addr: None,
    })
    .await
    .unwrap();

    // One registry for both kinds is what lets the proxy route to either
    // without knowing the difference.
    let got = reg.get("cache-0").await.unwrap();
    assert_eq!(got.kind, SinkKind::Cache);
    assert_eq!(
        reg.deliver_to("cache-0", Some("proxy:5000")).await,
        vec!["proxy:5000".to_string()]
    );
}
