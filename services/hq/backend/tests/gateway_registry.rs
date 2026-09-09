//! The gateway's connection registry and request correlation.
//!
//! The registry is the authoritative view of what this replica holds; Redis is
//! only a projection of it. These cover the parts that decide whether a
//! dispatch lands or is silently lost.

use chrono::Utc;
use hq_backend::gateway::registry::Registry;
use hq_types::hq::TapId;
use hq_types::{OnlineTapState, TapName};
use zakofish4_common::event::RequestOutcome;
use zakofish4_common::messages::{AudioMetadataSuccessMessage, ResponseVariant};
use zakofish4_common::model::{AudioCachePolicy, AudioCacheType, RequestId};

fn state(tap: &str, connection_id: u64) -> OnlineTapState {
    OnlineTapState {
        tap_id: TapId(tap.into()),
        tap_name: TapName("t".into()),
        connection_id,
        friendly_name: format!("c{connection_id}"),
        selection_weight: 1.0,
        connected_at: Utc::now(),
        replica_id: "r1".into(),
    }
}

fn answered() -> RequestOutcome {
    RequestOutcome::Answered(ResponseVariant::AudioMetadataSuccess(
        AudioMetadataSuccessMessage {
            metadatas: vec![],
            cache: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
        },
    ))
}

#[test]
fn connection_ids_are_unique_within_a_replica() {
    let registry = Registry::new();
    let a = registry.next_connection_id();
    let b = registry.next_connection_id();
    let c = registry.next_connection_id();
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert!(b > a && c > b);
}

#[test]
fn an_empty_registry_reports_nothing_online() {
    let registry = Registry::new();
    assert!(registry.is_empty());
    assert!(registry.states_for(&TapId("tap-1".into())).is_empty());
    assert!(registry.tap_ids().is_empty());
}

#[test]
fn a_waiter_receives_the_outcome_it_is_waiting_for() {
    let registry = Registry::new();
    let id = RequestId::random();

    let mut rx = registry.await_request(id);
    assert!(registry.complete(id, answered()));

    let got = rx.try_recv().expect("the waiter should have been woken");
    assert!(matches!(got, RequestOutcome::Answered(_)));
}

/// An audio request completes twice: once when the tap answers, and again when
/// the transfer ends. Only the first has a caller blocked on it, so the second
/// finding no waiter is normal — it goes to history, not to an error path.
#[test]
fn a_second_completion_finds_no_waiter_and_says_so() {
    let registry = Registry::new();
    let id = RequestId::random();

    let _rx = registry.await_request(id);
    assert!(registry.complete(id, answered()), "first completion has a waiter");
    assert!(
        !registry.complete(id, RequestOutcome::Streamed(
            zakofish4_common::model::StreamOutcome::Completed { frames_sent: 10 }
        )),
        "the stream report arrives after the caller was released"
    );
}

#[test]
fn completing_an_unknown_request_is_not_an_error() {
    let registry = Registry::new();
    assert!(!registry.complete(RequestId::random(), answered()));
}

/// A caller that gives up must not leave its slot behind, or the map grows
/// without bound under load.
#[test]
fn forgetting_a_request_releases_its_slot() {
    let registry = Registry::new();
    let id = RequestId::random();

    let _rx = registry.await_request(id);
    assert_eq!(registry.pending_len(), 1);

    registry.forget(id);
    assert_eq!(registry.pending_len(), 0);
    assert!(!registry.complete(id, answered()));
}

/// A dropped receiver means the caller went away first. That is not an error,
/// but it must be reported as undelivered so the caller side can tell.
#[test]
fn completing_after_the_caller_vanished_reports_undelivered() {
    let registry = Registry::new();
    let id = RequestId::random();

    let rx = registry.await_request(id);
    drop(rx);

    assert!(!registry.complete(id, answered()));
    assert_eq!(registry.pending_len(), 0);
}

#[test]
fn waiters_for_different_requests_do_not_interfere() {
    let registry = Registry::new();
    let a = RequestId::random();
    let b = RequestId::random();

    let mut rx_a = registry.await_request(a);
    let mut rx_b = registry.await_request(b);

    registry.complete(b, answered());

    assert!(rx_a.try_recv().is_err(), "a must still be waiting");
    assert!(rx_b.try_recv().is_ok(), "b must have been woken");
}

#[test]
fn removing_an_absent_connection_is_harmless() {
    let registry = Registry::new();
    assert!(registry.remove(999).is_none());
    assert!(registry.get(999).is_none());
}

/// A live `Connection` needs a `TapHandle`, which only the driver can make —
/// so the population case is covered by the driver's own tests, and this pins
/// the shape the presence projection reads.
#[test]
fn states_are_looked_up_per_tap() {
    let registry = Registry::new();
    let s = state("tap-1", 1);
    assert_eq!(s.replica_id, "r1");
    assert!(registry.states_for(&s.tap_id).is_empty());
}
