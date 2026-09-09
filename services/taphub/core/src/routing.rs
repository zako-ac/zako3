//! Connection selection.
//!
//! Moved to `zako3-tap-select` so the HQ gateway uses the same logic — and so
//! the sanitising of self-reported weights lives in exactly one place.
pub use zako3_tap_select::DynamicSampler;
