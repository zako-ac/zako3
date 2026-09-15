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
        self.next_state_with(states, |_| Some(1.0))
    }

    /// Pick a connection, skipping the ones the caller rules out and scaling
    /// the weights of the ones it wants deprioritised.
    ///
    /// `adjust` answers per connection: `None` rules it out for this draw, and
    /// `Some(scale)` multiplies its sanitised weight — `0.0` deprioritises it
    /// without excluding it. The two are deliberately different. A connection
    /// that just failed mid-request must not be picked again, so it is ruled
    /// out; a tap that is merely busy must still be reachable, because it is
    /// the only tap that can serve its own id and there is nothing to fall back
    /// to.
    ///
    /// The cursor advances over the *eligible* set, so a ruled-out connection
    /// does not leave a hole in the weight space that has to be redistributed.
    pub fn next_state_with<'a>(
        &mut self,
        states: &'a OnlineTapStates,
        adjust: impl Fn(&OnlineTapState) -> Option<f32>,
    ) -> Option<&'a OnlineTapState> {
        let eligible: Vec<(&OnlineTapState, f64)> = states
            .iter()
            .filter_map(|s| {
                let scale = adjust(s)?;
                // A caller-supplied scale is as untrusted as the reported
                // weight: `NaN` would make every comparison false and negative
                // weights would silently cancel each other out.
                let scale = if scale.is_finite() {
                    scale.clamp(0.0, MAX_WEIGHT)
                } else {
                    0.0
                };
                Some((s, sanitize_weight(s.selection_weight) as f64 * scale as f64))
            })
            .collect();

        if eligible.is_empty() {
            return None;
        }

        let total: f64 = eligible.iter().map(|(_, w)| *w).sum();
        // Every eligible connection asked for zero, or the tap is marked busy
        // and every weight was scaled away. Falling back to uniform keeps the
        // tap usable instead of unreachable. Weights and scales are both
        // clamped to finite non-negative values before they get here, so this
        // really is a test for "nothing is left to prefer", not for NaN.
        if total <= 0.0 {
            let idx = self.advance_cursor(eligible.len());
            return eligible.get(idx).map(|(s, _)| *s);
        }

        const PHI: f64 = 0.618_033_988_749_895;
        self.cursor = (self.cursor + PHI) % 1.0;
        let target = self.cursor * total;

        let mut running = 0.0;
        for (s, w) in &eligible {
            running += w;
            if running >= target {
                return Some(*s);
            }
        }
        eligible.last().map(|(s, _)| *s)
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
