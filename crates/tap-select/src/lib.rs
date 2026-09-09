//! Picking one connection from a tap's live set.
//!
//! Lifted out of taphub so the HQ gateway can use the same selection, and so
//! the weight sanitising below has one home rather than two.

use zako3_types::{OnlineTapState, OnlineTapStates};

/// Largest weight a tap may claim.
///
/// `selection_weight` is **self-reported** in `ClientHello` and fed straight
/// into the sampler, so it is attacker-controlled. Left unchecked,
/// `f32::INFINITY` makes the running sum reach the target at that element
/// immediately, handing one tap every request; `NaN` makes every comparison
/// false so the loop falls through to the last element, breaking selection
/// entirely. Both are clamped away here.
pub const MAX_WEIGHT: f32 = 1_000.0;

/// Coerce a reported weight into something usable.
///
/// Non-finite becomes the default, negatives become zero, and everything is
/// capped. Prefer taking the weight from the tap's own record where one exists;
/// this is the floor, not the ceiling, of the defence.
pub fn sanitize_weight(weight: f32) -> f32 {
    if !weight.is_finite() {
        return 1.0;
    }
    weight.clamp(0.0, MAX_WEIGHT)
}

/// Weighted pick over a tap's connections.
///
/// Advances a cursor by the golden ratio each call, which spreads consecutive
/// picks across the weight space instead of clustering the way independent
/// random draws do — with a handful of connections that difference is very
/// visible.
pub struct DynamicSampler {
    cursor: f64,
}

impl Default for DynamicSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicSampler {
    pub fn new() -> Self {
        Self { cursor: 0.5 }
    }

    /// Pick a connection, or `None` if the tap has none online.
    pub fn next_state<'a>(&mut self, states: &'a OnlineTapStates) -> Option<&'a OnlineTapState> {
        if states.is_empty() {
            return None;
        }

        let weights: Vec<f64> = states
            .iter()
            .map(|s| sanitize_weight(s.selection_weight) as f64)
            .collect();

        let total: f64 = weights.iter().sum();
        // Every connection asked for zero (or all were clamped to it). Falling
        // back to uniform keeps the tap usable instead of unreachable.
        if total <= 0.0 {
            let idx = self.advance_cursor(states.len());
            return states.get(idx);
        }

        const PHI: f64 = 0.618_033_988_749_895;
        self.cursor = (self.cursor + PHI) % 1.0;
        let target = self.cursor * total;

        let mut running = 0.0;
        for (i, w) in weights.iter().enumerate() {
            running += w;
            if running >= target {
                return states.get(i);
            }
        }
        states.last()
    }

    /// Backwards-compatible shim for callers that only need the id.
    pub fn next_connection_id(&mut self, states: &OnlineTapStates) -> Option<u64> {
        self.next_state(states).map(|s| s.connection_id)
    }

    fn advance_cursor(&mut self, len: usize) -> usize {
        const PHI: f64 = 0.618_033_988_749_895;
        self.cursor = (self.cursor + PHI) % 1.0;
        ((self.cursor * len as f64) as usize).min(len - 1)
    }
}
