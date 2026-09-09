//! Weighted selection, and the hostile-input cases the old sampler mishandled.

use std::collections::HashMap;

use chrono::Utc;
use zako3_tap_select::{DynamicSampler, MAX_WEIGHT, sanitize_weight};
use zako3_types::hq::TapId;
use zako3_types::{OnlineTapState, OnlineTapStates, TapName};

fn conn(connection_id: u64, weight: f32) -> OnlineTapState {
    OnlineTapState {
        tap_id: TapId("tap-1".into()),
        tap_name: TapName("test".into()),
        connection_id,
        friendly_name: format!("c{connection_id}"),
        selection_weight: weight,
        connected_at: Utc::now(),
        replica_id: "r1".into(),
    }
}

/// Pick many times and count where they landed.
fn distribution(states: &OnlineTapStates, draws: usize) -> HashMap<u64, usize> {
    let mut sampler = DynamicSampler::new();
    let mut counts = HashMap::new();
    for _ in 0..draws {
        let picked = sampler.next_state(states).expect("a connection");
        *counts.entry(picked.connection_id).or_insert(0) += 1;
    }
    counts
}

#[test]
fn no_connections_means_no_pick() {
    let mut sampler = DynamicSampler::new();
    assert!(sampler.next_state(&vec![]).is_none());
}

#[test]
fn a_single_connection_always_wins() {
    let states = vec![conn(1, 1.0)];
    assert_eq!(distribution(&states, 50)[&1], 50);
}

#[test]
fn equal_weights_split_roughly_evenly() {
    let states = vec![conn(1, 1.0), conn(2, 1.0)];
    let counts = distribution(&states, 1000);
    let a = counts[&1] as f64;
    let b = counts[&2] as f64;
    assert!((a - b).abs() / 1000.0 < 0.1, "expected an even split, got {counts:?}");
}

#[test]
fn a_heavier_connection_is_picked_more_often() {
    let states = vec![conn(1, 1.0), conn(2, 3.0)];
    let counts = distribution(&states, 1000);
    assert!(
        counts[&2] > counts[&1] * 2,
        "weight 3 should clearly beat weight 1, got {counts:?}"
    );
}

/// `selection_weight` is self-reported by the tap, so these are attacker
/// inputs, not edge cases.
#[test]
fn infinite_weight_cannot_capture_all_traffic() {
    let states = vec![conn(1, 1.0), conn(2, f32::INFINITY)];
    let counts = distribution(&states, 1000);
    assert!(
        counts.get(&1).copied().unwrap_or(0) > 0,
        "an honest tap must still be reachable, got {counts:?}"
    );
}

#[test]
fn nan_weight_does_not_break_selection() {
    let states = vec![conn(1, f32::NAN), conn(2, 1.0)];
    let counts = distribution(&states, 1000);
    // The old sampler fell through to the last element on every draw.
    assert!(
        counts.get(&1).copied().unwrap_or(0) > 0,
        "a NaN weight must not make its own connection unreachable, got {counts:?}"
    );
    assert_eq!(counts.values().sum::<usize>(), 1000);
}

#[test]
fn negative_weight_is_treated_as_zero_not_as_a_credit() {
    assert_eq!(sanitize_weight(-5.0), 0.0);
    let states = vec![conn(1, -100.0), conn(2, 1.0)];
    let counts = distribution(&states, 500);
    assert_eq!(counts.get(&2).copied().unwrap_or(0), 500, "{counts:?}");
}

#[test]
fn weights_are_capped() {
    assert_eq!(sanitize_weight(f32::MAX), MAX_WEIGHT);
    assert_eq!(sanitize_weight(f32::INFINITY), 1.0);
    assert_eq!(sanitize_weight(f32::NEG_INFINITY), 1.0);
    assert_eq!(sanitize_weight(f32::NAN), 1.0);
    assert_eq!(sanitize_weight(2.5), 2.5);
}

/// All-zero weights would otherwise make a tap that is plainly online look
/// unreachable.
#[test]
fn all_zero_weights_fall_back_to_uniform() {
    let states = vec![conn(1, 0.0), conn(2, 0.0), conn(3, 0.0)];
    let counts = distribution(&states, 600);
    assert_eq!(counts.values().sum::<usize>(), 600);
    assert_eq!(counts.len(), 3, "every connection should be reachable: {counts:?}");
}

/// Selection has to return the whole state now, not just an id: with a
/// multi-replica gateway the caller needs to know *which replica* holds the
/// connection it just picked.
#[test]
fn selection_reports_the_owning_replica() {
    let mut a = conn(1, 1.0);
    a.replica_id = "replica-a".into();
    let states = vec![a];
    let mut sampler = DynamicSampler::new();
    let picked = sampler.next_state(&states).unwrap();
    assert_eq!(picked.replica_id, "replica-a");
    assert_eq!(picked.connection_id, 1);
}
