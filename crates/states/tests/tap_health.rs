//! The tap health store, against an in-memory repository.
//!
//! Two properties this has to get right and that nothing else can check: a
//! verdict survives only as long as the replica that wrote it keeps writing,
//! and a verdict written by one replica is visible to every other one — because
//! a routing decision made on divergent state is exactly the failure the store
//! exists to prevent.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use zako3_states::cache_repo::{CacheRepository, CacheRepositoryRef};
use zako3_states::error::Result;
use zako3_states::{
    ConnectionHealth, ConnectionVerdict, TapHealthService, TapVerdict,
};
use zako3_types::hq::TapId;

/// A repository with an explicit "expire this key" lever, so a lapsed lease can
/// be simulated without sleeping through a real TTL.
#[derive(Default)]
struct MemRepo {
    strings: Mutex<HashMap<String, String>>,
    sets: Mutex<HashMap<String, HashSet<String>>>,
}

impl MemRepo {
    fn expire(&self, key: &str) {
        self.strings.lock().unwrap().remove(key);
    }
}

#[async_trait]
impl CacheRepository for MemRepo {
    async fn get(&self, key: &str) -> Option<String> {
        self.strings.lock().unwrap().get(key).cloned()
    }
    async fn set(&self, key: &str, value: &str) {
        self.strings
            .lock()
            .unwrap()
            .insert(key.into(), value.into());
    }
    async fn set_ex(&self, key: &str, value: &str, _ttl: u64) {
        self.set(key, value).await;
    }
    async fn del(&self, key: &str) {
        self.strings.lock().unwrap().remove(key);
    }
    async fn incr(&self, _k: &str) -> Result<i64> {
        Ok(0)
    }
    async fn decr(&self, _k: &str) -> Result<i64> {
        Ok(0)
    }
    async fn incrby(&self, _k: &str, _a: i64) -> Result<i64> {
        Ok(0)
    }
    async fn pfadd(&self, _k: &str, _e: &str) -> Result<()> {
        Ok(())
    }
    async fn pfcount(&self, _k: &str) -> Result<u64> {
        Ok(0)
    }
    async fn pfcount_multi(&self, _k: &[String]) -> Result<u64> {
        Ok(0)
    }
    async fn sadd(&self, key: &str, member: &str) -> Result<()> {
        self.sets
            .lock()
            .unwrap()
            .entry(key.into())
            .or_default()
            .insert(member.into());
        Ok(())
    }
    async fn srem(&self, key: &str, member: &str) -> Result<()> {
        if let Some(set) = self.sets.lock().unwrap().get_mut(key) {
            set.remove(member);
        }
        Ok(())
    }
    async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        Ok(self
            .sets
            .lock()
            .unwrap()
            .get(key)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default())
    }
    async fn hgetall(&self, _k: &str) -> Result<Vec<(String, String)>> {
        Ok(vec![])
    }
    async fn hincrby(&self, _k: &str, _f: &str, _a: i64) -> Result<i64> {
        Ok(0)
    }
    async fn hdel_key(&self, _k: &str) -> Result<()> {
        Ok(())
    }
}

fn tap() -> TapId {
    TapId("tap-1".into())
}

fn probed(connection_id: u64, verdict: ConnectionVerdict, ms: Option<u64>) -> ConnectionHealth {
    ConnectionHealth {
        connection_id,
        verdict,
        time_to_first_sample_ms: ms,
        reason: None,
        probed_at: Utc::now(),
    }
}

fn setup() -> (Arc<MemRepo>, TapHealthService) {
    let repo = Arc::new(MemRepo::default());
    let svc = TapHealthService::new(repo.clone() as CacheRepositoryRef);
    (repo, svc)
}

/// Before anything probes a tap — which is every tap the day this ships — the
/// answer has to be "no opinion", not "broken".
#[tokio::test]
async fn an_unprobed_tap_is_unknown_and_usable() {
    let (_repo, svc) = setup();
    let view = svc.get(&tap()).await.unwrap();

    assert_eq!(view.verdict(), TapVerdict::Unknown);
    assert!(view.is_usable());
    assert!(view.excluded_connections().is_empty());
    assert_eq!(view.weight_scale("r1", 1), 1.0);
}

#[tokio::test]
async fn a_failed_probe_excludes_only_the_connection_that_failed() {
    let (_repo, svc) = setup();
    svc.publish_probe(
        &tap(),
        "r1",
        &[
            probed(1, ConnectionVerdict::Failed, None),
            probed(2, ConnectionVerdict::Healthy, Some(120)),
        ],
    )
    .await
    .unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert!(view.is_excluded("r1", 1));
    assert!(!view.is_excluded("r1", 2));
    assert_eq!(view.excluded_connections(), vec![("r1".to_string(), 1)]);
    // One good connection is enough: the tap is still worth routing to.
    assert!(view.is_usable());
    assert_eq!(view.verdict(), TapVerdict::Healthy);
    assert_eq!(view.time_to_first_sample_ms(), Some(120));
}

/// Only when every connection has been judged bad does the tap stop being worth
/// a request.
#[tokio::test]
async fn a_tap_whose_every_connection_failed_is_unhealthy() {
    let (_repo, svc) = setup();
    svc.publish_probe(
        &tap(),
        "r1",
        &[
            probed(1, ConnectionVerdict::Failed, None),
            probed(2, ConnectionVerdict::Failed, None),
        ],
    )
    .await
    .unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert_eq!(view.verdict(), TapVerdict::Unhealthy);
    assert!(!view.is_usable());
}

/// A tap that has not implemented the probe has told us nothing, which is not
/// the same as telling us it is broken.
#[tokio::test]
async fn an_unsupported_probe_is_not_a_failure() {
    let (_repo, svc) = setup();
    svc.publish_probe(&tap(), "r1", &[probed(1, ConnectionVerdict::Unsupported, None)])
        .await
        .unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert!(view.is_usable());
    assert!(!view.is_excluded("r1", 1));
    assert_eq!(view.verdict(), TapVerdict::Healthy);
}

/// Failure counts persist across rounds, so a tap that keeps refusing stays
/// judged rather than resetting to "unknown" every time.
#[tokio::test]
async fn consecutive_failures_accumulate() {
    let (_repo, svc) = setup();
    for expected in 1..=3 {
        svc.publish_probe(&tap(), "r1", &[probed(1, ConnectionVerdict::Failed, None)])
            .await
            .unwrap();
        assert_eq!(svc.get(&tap()).await.unwrap().consecutive_failures(), expected);
    }

    // And a healthy round clears the run.
    svc.publish_probe(&tap(), "r1", &[probed(1, ConnectionVerdict::Healthy, Some(90))])
        .await
        .unwrap();
    assert_eq!(svc.get(&tap()).await.unwrap().consecutive_failures(), 0);
}

/// The reason the record is per-replica at all: two replicas each hold
/// connections for the tap, and a router that cannot see both makes a different
/// decision depending on which one it asked.
#[tokio::test]
async fn every_replicas_verdict_is_visible_to_every_reader() {
    let (_repo, svc) = setup();
    svc.publish_probe(&tap(), "a", &[probed(1, ConnectionVerdict::Healthy, Some(80))])
        .await
        .unwrap();
    svc.publish_probe(&tap(), "b", &[probed(2, ConnectionVerdict::Failed, None)])
        .await
        .unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert_eq!(view.records().len(), 2);
    // Keyed by replica as well as id: connection 1 belongs to "a" and to nobody
    // else, even though "b" may have a connection numbered 1 too.
    assert!(view.is_excluded("b", 2));
    assert!(!view.is_excluded("a", 2));
    assert!(!view.is_excluded("a", 1));
}

/// A crashed replica cannot retract its verdict, so the lease has to.
#[tokio::test]
async fn a_lapsed_verdict_disappears_and_is_pruned_from_the_index() {
    let (repo, svc) = setup();
    svc.publish_probe(&tap(), "a", &[probed(1, ConnectionVerdict::Failed, None)])
        .await
        .unwrap();
    svc.publish_probe(&tap(), "b", &[probed(2, ConnectionVerdict::Healthy, Some(70))])
        .await
        .unwrap();

    // Replica "a" stops refreshing.
    repo.expire("tap:health:tap-1:a");

    let view = svc.get(&tap()).await.unwrap();
    assert_eq!(view.records().len(), 1);
    assert!(!view.is_excluded("a", 1), "a dead replica's word is not evidence");
    assert!(view.is_usable());

    // And the index no longer names it, so later reads cost nothing.
    assert_eq!(repo.smembers("tap:health:tap-1").await.unwrap(), vec!["b".to_string()]);
}

#[tokio::test]
async fn an_empty_probe_withdraws_that_replica_rather_than_recording_a_verdict() {
    let (_repo, svc) = setup();
    svc.publish_probe(&tap(), "a", &[probed(1, ConnectionVerdict::Failed, None)])
        .await
        .unwrap();
    svc.publish_probe(&tap(), "b", &[probed(2, ConnectionVerdict::Healthy, Some(70))])
        .await
        .unwrap();

    // "a" no longer holds a connection for this tap.
    svc.publish_probe(&tap(), "a", &[]).await.unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert_eq!(view.records().len(), 1);
    assert_eq!(view.records()[0].replica_id, "b");
}

/// A first sample that never arrives is a failed request for the listener, so
/// it deprioritises the tap at once rather than after a run of them.
#[tokio::test]
async fn a_missing_first_sample_marks_the_tap_busy_immediately() {
    let (_repo, svc) = setup();
    let busy = svc.record_first_sample(&tap(), None).await.unwrap();

    assert!(busy);
    let view = svc.get(&tap()).await.unwrap();
    assert!(view.is_busy());
    assert_eq!(view.verdict(), TapVerdict::Busy);
    // Busy is not unhealthy: it is the only tap that can serve its own id.
    assert!(view.is_usable());
    assert_eq!(view.weight_scale("r1", 1), 0.0);
}

/// One slow start is a cold cache. A run of them is a tap that cannot keep up.
#[tokio::test]
async fn slow_samples_only_make_a_tap_busy_once_they_keep_happening() {
    let (_repo, svc) = setup();
    let slow = svc.slow_first_sample_ms() + 1;

    assert!(!svc.record_first_sample(&tap(), Some(slow)).await.unwrap());
    assert!(!svc.record_first_sample(&tap(), Some(slow)).await.unwrap());
    assert!(
        svc.record_first_sample(&tap(), Some(slow)).await.unwrap(),
        "three in a row is a tap that keeps doing this"
    );

    let view = svc.get(&tap()).await.unwrap();
    assert!(view.is_busy());
    // And the observed latency is visible, not just the verdict.
    assert_eq!(view.time_to_first_sample_ms(), Some(slow));
}

#[tokio::test]
async fn a_fast_sample_clears_a_busy_verdict() {
    let (_repo, svc) = setup();
    svc.record_first_sample(&tap(), None).await.unwrap();
    assert!(svc.get(&tap()).await.unwrap().is_busy());

    let fast = svc.slow_first_sample_ms() / 4;
    svc.record_first_sample(&tap(), Some(fast)).await.unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert!(!view.is_busy());
    assert_eq!(view.weight_scale("r1", 1), 1.0);
}

/// The busy stamp is what expires, not the lease: a tap that was slow once must
/// be tried again soon, not written off.
#[tokio::test]
async fn a_busy_verdict_expires_on_its_own() {
    let repo = Arc::new(MemRepo::default());
    let svc = TapHealthService::new(repo as CacheRepositoryRef).with_busy_ttl_secs(1);

    svc.record_first_sample(&tap(), None).await.unwrap();
    assert!(svc.get(&tap()).await.unwrap().is_busy());

    tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;

    let view = svc.get(&tap()).await.unwrap();
    assert!(!view.is_busy());
    assert!(view.busy_until().is_none());
    assert_eq!(view.weight_scale("r1", 1), 1.0);
}

#[tokio::test]
async fn a_slow_connection_is_deprioritised_without_being_excluded() {
    let (_repo, svc) = setup();
    svc.publish_probe(
        &tap(),
        "r1",
        &[
            probed(1, ConnectionVerdict::Slow, Some(9_000)),
            probed(2, ConnectionVerdict::Healthy, Some(90)),
        ],
    )
    .await
    .unwrap();

    let view = svc.get(&tap()).await.unwrap();
    assert!(!view.is_excluded("r1", 1), "slow is not broken");
    assert_eq!(view.weight_scale("r1", 1), 0.0);
    assert_eq!(view.weight_scale("r1", 2), 1.0);
    // The best of the two is what the operators see.
    assert_eq!(view.time_to_first_sample_ms(), Some(90));
}
