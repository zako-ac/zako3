//! Gateway presence across replicas, against an in-memory repository.
//!
//! The property under test is the one `TapHubStateService` cannot provide: two
//! processes publishing for the same tap must both be visible, and a process
//! that stops refreshing must disappear on its own.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use zako3_states::cache_repo::{CacheRepository, CacheRepositoryRef};
use zako3_states::error::Result;
use zako3_states::GatewayPresenceService;
use zako3_types::hq::TapId;
use zako3_types::{OnlineTapState, OnlineTapStates, TapName};

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

fn state(replica: &str, connection_id: u64, weight: f32) -> OnlineTapState {
    OnlineTapState {
        tap_id: tap(),
        tap_name: TapName("test-tap".into()),
        connection_id,
        friendly_name: format!("conn-{connection_id}"),
        selection_weight: weight,
        connected_at: Utc::now(),
        replica_id: replica.into(),
    }
}

fn setup() -> (Arc<MemRepo>, GatewayPresenceService) {
    let repo = Arc::new(MemRepo::default());
    let svc = GatewayPresenceService::new(repo.clone() as CacheRepositoryRef);
    (repo, svc)
}

fn ids(mut states: OnlineTapStates) -> Vec<(String, u64)> {
    states.sort_by_key(|s| (s.replica_id.clone(), s.connection_id));
    states
        .into_iter()
        .map(|s| (s.replica_id, s.connection_id))
        .collect()
}

#[tokio::test]
async fn one_replica_publishes_and_reads_back() {
    let (_repo, svc) = setup();
    svc.publish(&tap(), "a", &vec![state("a", 1, 1.0)]).await.unwrap();
    assert_eq!(ids(svc.get(&tap()).await.unwrap()), vec![("a".into(), 1)]);
}

/// The whole point. `publish_tap_states` writes one key for the entire tap, so
/// two replicas doing that would overwrite each other and only the last writer
/// would be reachable.
#[tokio::test]
async fn two_replicas_are_both_visible() {
    let (_repo, svc) = setup();
    svc.publish(&tap(), "a", &vec![state("a", 1, 1.0)]).await.unwrap();
    svc.publish(&tap(), "b", &vec![state("b", 2, 1.0)]).await.unwrap();

    assert_eq!(
        ids(svc.get(&tap()).await.unwrap()),
        vec![("a".into(), 1), ("b".into(), 2)]
    );
}

#[tokio::test]
async fn a_replica_can_hold_several_connections_for_one_tap() {
    let (_repo, svc) = setup();
    svc.publish(
        &tap(),
        "a",
        &vec![state("a", 1, 1.0), state("a", 2, 2.0)],
    )
    .await
    .unwrap();

    assert_eq!(
        ids(svc.get(&tap()).await.unwrap()),
        vec![("a".into(), 1), ("a".into(), 2)]
    );
}

/// A replica that dies stops refreshing; its lease lapses and the next read
/// prunes it. Nothing has to run on the way down, which is the point — a
/// crashed process cannot clean up after itself.
#[tokio::test]
async fn a_lapsed_lease_disappears_and_is_pruned_from_the_index() {
    let (repo, svc) = setup();
    svc.publish(&tap(), "a", &vec![state("a", 1, 1.0)]).await.unwrap();
    svc.publish(&tap(), "b", &vec![state("b", 2, 1.0)]).await.unwrap();

    // Replica "a" stops refreshing.
    repo.expire("tap:gw:tap-1:a");

    assert_eq!(ids(svc.get(&tap()).await.unwrap()), vec![("b".into(), 2)]);

    // And the index no longer names it, so later reads cost nothing.
    let members = repo.smembers("tap:gw:tap-1").await.unwrap();
    assert_eq!(members, vec!["b".to_string()]);
}

#[tokio::test]
async fn publishing_an_empty_list_withdraws_that_replica_only() {
    let (_repo, svc) = setup();
    svc.publish(&tap(), "a", &vec![state("a", 1, 1.0)]).await.unwrap();
    svc.publish(&tap(), "b", &vec![state("b", 2, 1.0)]).await.unwrap();

    svc.publish(&tap(), "a", &vec![]).await.unwrap();

    assert_eq!(ids(svc.get(&tap()).await.unwrap()), vec![("b".into(), 2)]);
}

#[tokio::test]
async fn withdrawing_removes_only_the_named_replica() {
    let (_repo, svc) = setup();
    svc.publish(&tap(), "a", &vec![state("a", 1, 1.0)]).await.unwrap();
    svc.publish(&tap(), "b", &vec![state("b", 2, 1.0)]).await.unwrap();

    svc.withdraw(&tap(), "a").await.unwrap();

    assert_eq!(ids(svc.get(&tap()).await.unwrap()), vec![("b".into(), 2)]);
}

#[tokio::test]
async fn an_unknown_tap_has_no_connections() {
    let (_repo, svc) = setup();
    assert!(svc.get(&TapId("nobody".into())).await.unwrap().is_empty());
}

/// A republish replaces that replica's list rather than appending to it, so a
/// connection that closed does not linger until its lease expires.
#[tokio::test]
async fn republishing_replaces_a_replicas_list() {
    let (_repo, svc) = setup();
    svc.publish(
        &tap(),
        "a",
        &vec![state("a", 1, 1.0), state("a", 2, 1.0)],
    )
    .await
    .unwrap();
    svc.publish(&tap(), "a", &vec![state("a", 2, 1.0)]).await.unwrap();

    assert_eq!(ids(svc.get(&tap()).await.unwrap()), vec![("a".into(), 2)]);
}
