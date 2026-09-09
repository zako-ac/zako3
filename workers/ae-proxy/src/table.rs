//! The routing table: `request_id → (tap, sink)`, and nothing else.
//!
//! Deliberately transport-free so the parts worth being sure about — which side
//! a datagram came from, what gets pinned, what is refused — are testable
//! without sockets or Redis.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use protofish4::RequestId;

/// Where a datagram should go, and why.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Forward to the sink — this came from the tap.
    ToSink(SocketAddr),
    /// Forward to the tap — this came from the sink.
    ToTap(SocketAddr),
    /// The route is being looked up. Drop this one; the sender will retry.
    Pending,
    /// Ask for the route, then drop this datagram.
    Lookup,
    /// Nothing to do with it.
    Drop(DropReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// No route for this request id — a stray or a guessed one.
    NoRoute,
    /// Both ends are already pinned and this is neither of them.
    NotAParty,
    /// The sink has not answered yet, so there is nowhere to send its reply.
    NoTapYet,
    /// The table is full.
    Full,
}

struct Route {
    sink: SocketAddr,
    /// Learned from the first datagram and then fixed. Without pinning, anyone
    /// who guessed a request id could rebind the mapping and redirect every
    /// sink→tap control packet — which is to say, all of a transfer's NACKs.
    tap: Option<SocketAddr>,
    /// Learned the same way, from the first datagram that is not the tap.
    ///
    /// Not compared against `sink`: the address the route was resolved to is a
    /// service address, and a reply comes from the pod that actually sent it.
    /// Requiring equality there would classify every legitimate `Ack` and
    /// `Nack` as a tap packet and drop it — silently, and only in the cluster.
    sink_reply: Option<SocketAddr>,
    last_seen: Instant,
}

enum Entry {
    /// A lookup is in flight. Timestamped so a Redis outage that loses the
    /// answer cannot wedge an id as permanently un-routable.
    Resolving(Instant),
    Ready(Route),
    /// Remembered as unroutable until this instant.
    Unknown(Instant),
}

pub struct RouteTable {
    entries: HashMap<RequestId, Entry>,
    negative: usize,
    max_routes: usize,
    max_negative: usize,
    idle: Duration,
    negative_ttl: Duration,
}

/// What a sweep removed, for the eviction metric.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub idle: usize,
    pub negative: usize,
}

impl RouteTable {
    pub fn new(
        max_routes: usize,
        max_negative: usize,
        idle: Duration,
        negative_ttl: Duration,
    ) -> Self {
        Self {
            entries: HashMap::new(),
            negative: 0,
            max_routes,
            max_negative,
            idle,
            negative_ttl,
        }
    }

    pub fn live_routes(&self) -> usize {
        self.entries.len() - self.negative
    }

    /// Decide what to do with a datagram, learning addresses as it goes.
    pub fn route(&mut self, id: RequestId, from: SocketAddr, now: Instant) -> Decision {
        match self.entries.get_mut(&id) {
            None => {
                if self.live_routes() >= self.max_routes {
                    return Decision::Drop(DropReason::Full);
                }
                self.entries.insert(id, Entry::Resolving(now));
                Decision::Lookup
            }
            Some(Entry::Resolving(_)) => Decision::Pending,
            Some(Entry::Unknown(until)) => {
                if now >= *until {
                    self.negative -= 1;
                    self.entries.insert(id, Entry::Resolving(now));
                    Decision::Lookup
                } else {
                    Decision::Drop(DropReason::NoRoute)
                }
            }
            Some(Entry::Ready(route)) => {
                route.last_seen = now;
                match (route.tap, route.sink_reply) {
                    (Some(tap), _) if tap == from => Decision::ToSink(route.sink),
                    (_, Some(reply)) if reply == from => match route.tap {
                        Some(tap) => Decision::ToTap(tap),
                        None => Decision::Drop(DropReason::NoTapYet),
                    },
                    // Learn-once, in the order the two ends actually speak: the
                    // tap always sends first, so it is pinned before a sink can
                    // reply. A third party that guessed the id and beat the
                    // sink to it costs this transfer its NACKs and nothing else.
                    (None, _) => {
                        route.tap = Some(from);
                        Decision::ToSink(route.sink)
                    }
                    (Some(tap), None) => {
                        route.sink_reply = Some(from);
                        Decision::ToTap(tap)
                    }
                    _ => Decision::Drop(DropReason::NotAParty),
                }
            }
        }
    }

    /// Record the result of a lookup started by [`Decision::Lookup`].
    pub fn resolved(&mut self, id: RequestId, sink: Option<SocketAddr>, now: Instant) {
        match sink {
            Some(sink) => {
                self.entries.insert(
                    id,
                    Entry::Ready(Route {
                        sink,
                        tap: None,
                        sink_reply: None,
                        last_seen: now,
                    }),
                );
            }
            None => {
                if self.negative >= self.max_negative {
                    // Refusing to remember is better than evicting a live
                    // transfer to make room for a guessed id.
                    self.entries.remove(&id);
                    return;
                }
                self.negative += 1;
                self.entries.insert(id, Entry::Unknown(now + self.negative_ttl));
            }
        }
    }

    /// Drop idle routes and expired negative entries.
    pub fn sweep(&mut self, now: Instant) -> Swept {
        let mut swept = Swept::default();
        let idle = self.idle;
        self.entries.retain(|_, entry| match entry {
            Entry::Ready(r) => {
                let keep = now.duration_since(r.last_seen) < idle;
                if !keep {
                    swept.idle += 1;
                }
                keep
            }
            Entry::Unknown(until) => {
                let keep = now < *until;
                if !keep {
                    swept.negative += 1;
                }
                keep
            }
            // A lookup that never came back — the answer was lost rather than
            // negative. Held to the same deadline so it retries rather than
            // wedging.
            Entry::Resolving(since) => {
                let keep = now.duration_since(*since) < idle;
                if !keep {
                    swept.idle += 1;
                }
                keep
            }
        });
        self.negative -= swept.negative;
        swept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    fn table() -> RouteTable {
        RouteTable::new(4, 2, Duration::from_secs(60), Duration::from_secs(5))
    }

    fn id() -> RequestId {
        RequestId(uuid::Uuid::new_v4())
    }

    #[test]
    fn an_unknown_id_asks_for_a_lookup_once() {
        let mut t = table();
        let (id, now) = (id(), Instant::now());
        assert_eq!(t.route(id, addr("1.2.3.4:5"), now), Decision::Lookup);
        assert_eq!(
            t.route(id, addr("1.2.3.4:5"), now),
            Decision::Pending,
            "a second datagram must not start a second Redis round-trip"
        );
    }

    #[test]
    fn the_tap_is_pinned_on_its_first_datagram() {
        let mut t = table();
        let (id, now) = (id(), Instant::now());
        let sink = addr("10.0.0.7:4101");
        t.route(id, addr("1.2.3.4:5"), now);
        t.resolved(id, Some(sink), now);

        assert_eq!(t.route(id, addr("1.2.3.4:5"), now), Decision::ToSink(sink));
        assert_eq!(t.route(id, addr("1.2.3.4:5"), now), Decision::ToSink(sink));
    }

    /// A reply comes from the pod that sent it, not from the service address
    /// the route was resolved to. Requiring equality with `sink` would drop
    /// every `Ack` and `Nack` — in the cluster only, and silently.
    #[test]
    fn a_reply_from_a_different_address_than_the_sink_still_reaches_the_tap() {
        let mut t = table();
        let (id, now) = (id(), Instant::now());
        let tap = addr("1.2.3.4:5");
        let sink = addr("10.0.0.7:4101");
        let pod = addr("10.244.3.9:4101");

        t.route(id, tap, now);
        t.resolved(id, Some(sink), now);
        assert_eq!(t.route(id, tap, now), Decision::ToSink(sink));
        assert_eq!(t.route(id, pod, now), Decision::ToTap(tap));
        assert_eq!(t.route(id, pod, now), Decision::ToTap(tap));
    }

    /// Without pinning, anyone who guessed a request id could redirect every
    /// sink→tap control packet — which is all of a transfer's NACKs.
    #[test]
    fn a_third_party_cannot_rebind_a_pinned_route() {
        let mut t = table();
        let (id, now) = (id(), Instant::now());
        let tap = addr("1.2.3.4:5");
        let sink = addr("10.0.0.7:4101");

        t.route(id, tap, now);
        t.resolved(id, Some(sink), now);
        t.route(id, tap, now);
        t.route(id, addr("10.244.3.9:4101"), now);

        assert_eq!(
            t.route(id, addr("9.9.9.9:1234"), now),
            Decision::Drop(DropReason::NotAParty)
        );
        assert_eq!(t.route(id, tap, now), Decision::ToSink(sink));
    }

    #[test]
    fn an_unroutable_id_is_remembered_until_its_ttl() {
        let mut t = table();
        let (id, now) = (id(), Instant::now());
        t.route(id, addr("1.2.3.4:5"), now);
        t.resolved(id, None, now);

        assert_eq!(
            t.route(id, addr("1.2.3.4:5"), now),
            Decision::Drop(DropReason::NoRoute)
        );
        assert_eq!(
            t.route(id, addr("1.2.3.4:5"), now + Duration::from_secs(6)),
            Decision::Lookup,
            "past the TTL it is worth asking again"
        );
    }

    /// Negative entries are attacker-controlled insertions into the same table,
    /// so they get their own budget. Losing the negative cache is a far better
    /// outcome than evicting live transfers.
    #[test]
    fn a_spray_of_guessed_ids_cannot_evict_live_routes() {
        let mut t = table();
        let now = Instant::now();
        let live = id();
        t.route(live, addr("1.2.3.4:5"), now);
        t.resolved(live, Some(addr("10.0.0.7:4101")), now);

        for _ in 0..10 {
            let junk = id();
            t.route(junk, addr("9.9.9.9:1"), now);
            t.resolved(junk, None, now);
        }

        assert_eq!(
            t.route(live, addr("1.2.3.4:5"), now),
            Decision::ToSink(addr("10.0.0.7:4101"))
        );
    }

    #[test]
    fn the_table_refuses_new_routes_when_full() {
        let mut t = table();
        let now = Instant::now();
        for _ in 0..4 {
            let i = id();
            t.route(i, addr("1.2.3.4:5"), now);
            t.resolved(i, Some(addr("10.0.0.7:4101")), now);
        }
        assert_eq!(
            t.route(id(), addr("1.2.3.4:5"), now),
            Decision::Drop(DropReason::Full)
        );
    }

    #[test]
    fn idle_routes_are_swept_and_a_lost_lookup_does_not_wedge() {
        let mut t = table();
        let now = Instant::now();
        let live = id();
        t.route(live, addr("1.2.3.4:5"), now);
        t.resolved(live, Some(addr("10.0.0.7:4101")), now);
        let lost = id();
        t.route(lost, addr("1.2.3.4:5"), now);

        let later = now + Duration::from_secs(61);
        assert_eq!(t.sweep(later), Swept { idle: 2, negative: 0 });
        assert_eq!(t.live_routes(), 0);
        assert_eq!(
            t.route(lost, addr("1.2.3.4:5"), later),
            Decision::Lookup,
            "a lookup whose answer was lost must be retried, not wedged"
        );
    }
}
