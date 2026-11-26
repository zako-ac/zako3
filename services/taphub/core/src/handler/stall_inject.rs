//! A debug probe for the metadata retry path.
//!
//! `taphub_meta_retry_total` only moves when a connection is *on probation*
//! (it has been retired for not answering at least once) and then fails to
//! answer another metadata request inside its half budget. Left to production
//! traffic that is a rare conjunction, so the counter can sit at zero for weeks
//! and nobody can tell a working retry path from a broken one — which is what
//! happened after the instrumentation shipped (2026-11-26).
//!
//! Freezing the tap process does not reproduce it: a stopped process stops its
//! I/O loop too, the transport dies instead of going quiet, and taphub sees a
//! clean close rather than a wedged connection (2026-11-27 02:10 decision).
//! This probe wedges the connection from the side the counters observe: the
//! request is never sent, the chan opened for it is never answered, and the
//! connection is put on probation so the next request takes the retry path.
//!
//! `ZAKO_META_STALL_INJECT=<tap_id>[:<n>]`, unset by default. `n` (default 1) is
//! how many of that tap's connections are held silent, counted from the oldest
//! connection id — a connection id changes when a tap reconnects, and arming the
//! probe restarts the pod, so the value has to survive a reconnect to be usable.
//! Anything unmatched leaves every connection alone.

use std::sync::OnceLock;

/// Environment variable that arms the probe.
const ENV: &str = "ZAKO_META_STALL_INJECT";

/// What a request may be stalled on: which tap, and how many of its connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Probe {
    pub(crate) tap_id: String,
    /// Connections of that tap to hold silent, from the lowest connection id.
    pub(crate) connections: usize,
}

static PROBE: OnceLock<Option<Probe>> = OnceLock::new();

/// The armed probe, read once per process and logged when it is not empty.
pub(crate) fn probe() -> Option<&'static Probe> {
    PROBE
        .get_or_init(|| {
            let raw = std::env::var(ENV).unwrap_or_default();
            let probe = parse(&raw);
            match &probe {
                Some(p) => tracing::warn!(
                    env = ENV,
                    tap_id = %p.tap_id,
                    connections = p.connections,
                    "metadata stall probe armed: metadata requests on this tap's oldest connections are held open and never sent"
                ),
                None => tracing::debug!(env = ENV, "metadata stall probe not armed"),
            }
            probe
        })
        .as_ref()
}

/// Parse `<tap_id>[:<n>]`.
///
/// An empty value leaves the probe off. A non-numeric or zero `n` falls back to
/// one connection rather than failing the probe: this is a debugging aid, and a
/// typo should cost the count, not the whole window.
fn parse(spec: &str) -> Option<Probe> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let (tap_id, connections) = match spec.rsplit_once(':') {
        Some((tap_id, n)) => (tap_id, n.trim().parse::<usize>().ok().filter(|n| *n > 0)),
        None => (spec, None),
    };
    let tap_id = tap_id.trim();
    if tap_id.is_empty() {
        return None;
    }
    Some(Probe {
        tap_id: tap_id.to_string(),
        connections: connections.unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_value_arms_nothing() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
        assert_eq!(parse(":"), None);
    }

    #[test]
    fn a_tap_id_alone_stalls_one_connection() {
        assert_eq!(
            parse("340376482209599488"),
            Some(Probe {
                tap_id: "340376482209599488".to_string(),
                connections: 1,
            })
        );
    }

    #[test]
    fn a_count_after_the_tap_id_sets_how_many_are_stalled() {
        assert_eq!(
            parse(" 340376482209599488 : 3 "),
            Some(Probe {
                tap_id: "340376482209599488".to_string(),
                connections: 3,
            })
        );
    }

    /// A mis-typed count costs the count and not the window.
    #[test]
    fn a_bad_count_falls_back_to_one() {
        for spec in ["123:abc", "123:0", "123:-2"] {
            assert_eq!(
                parse(spec),
                Some(Probe {
                    tap_id: "123".to_string(),
                    connections: 1,
                }),
                "{spec}"
            );
        }
    }
}
