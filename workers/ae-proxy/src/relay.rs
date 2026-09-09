//! One socket, one loop, no key material.
//!
//! ## Why a single socket
//!
//! The tap's NAT keys its mapping on `(tap_source, proxy_destination)`. Replying
//! to it from a different socket — a per-sink ephemeral one, say — means a
//! symmetric NAT drops every sink→tap packet, which is to say every `Ack` and
//! every `Nack`, silently and only in production. So the proxy receives and
//! sends both directions on the one public socket, and demultiplexes by
//! `request_id` and source address instead.
//!
//! ## What it never does
//!
//! Never decrypts, never buffers payloads, never parses past the 22-byte
//! cleartext header. `Header::peek_request_id` tolerates packet kinds it does
//! not recognise, so new kinds can ship on the endpoints without redeploying
//! this.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use protofish4::proto::header::Header;
use tokio::net::{UdpSocket, lookup_host};
use tokio::sync::mpsc;

use crate::metrics;
use crate::table::{Decision, DropReason, RouteTable};

/// Where `request_id → sink` comes from. A trait so the relay is testable
/// without Redis.
#[async_trait]
pub trait RouteSource: Send + Sync + 'static {
    /// The sink's authority string, e.g. `"release-name-cache:4101"`.
    async fn lookup(&self, request_id: &uuid::Uuid) -> Option<String>;
}

#[async_trait]
impl RouteSource for zako3_states::SinkRegistry {
    async fn lookup(&self, request_id: &uuid::Uuid) -> Option<String> {
        self.lookup_route(request_id).await
    }
}

pub struct RelayConfig {
    pub max_routes: usize,
    pub max_negative: usize,
    /// Datagrams held across every in-flight route lookup, together.
    ///
    /// The first packets of a transfer arrive before the route is known, and a
    /// sender that opens with a burst can empty its whole send window into that
    /// window — leaving nothing to trigger the sink's first `Ack`, so the
    /// transfer dies of `NoAck` rather than losing a frame or two. Holding them
    /// for the length of one Redis read fixes that.
    ///
    /// One global budget rather than one per request id: the ids are
    /// attacker-chosen, so a per-id allowance multiplies straight into memory.
    pub max_pending: usize,
    pub route_idle: Duration,
    pub negative_ttl: Duration,
}

struct Resolution {
    id: protofish4::RequestId,
    sink: Option<SocketAddr>,
}

/// Read datagrams until the socket fails.
pub async fn run(
    socket: Arc<UdpSocket>,
    routes: Arc<dyn RouteSource>,
    cfg: RelayConfig,
) -> anyhow::Result<()> {
    let mut table = RouteTable::new(
        cfg.max_routes,
        cfg.max_negative,
        cfg.route_idle,
        cfg.negative_ttl,
    );
    // Lookups run off the datagram path. Doing them inline would let a spray of
    // guessed request ids stall the relay behind Redis round-trips.
    let (resolved_tx, mut resolved_rx) = mpsc::channel::<Resolution>(1024);
    let mut pending: VecDeque<Pending> = VecDeque::new();
    let mut sweeper = tokio::time::interval(Duration::from_secs(10));
    sweeper.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut buf = vec![0u8; 2048];
    loop {
        tokio::select! {
            res = socket.recv_from(&mut buf) => {
                let (len, from) = res?;
                let now = Instant::now();
                let Ok(id) = Header::peek_request_id(&buf[..len]) else {
                    metrics::DROPPED.with_label_values(&["malformed"]).inc();
                    continue;
                };
                match table.route(id, from, now) {
                    Decision::ToSink(to) => forward(&socket, &buf[..len], to, "to_sink").await,
                    Decision::ToTap(to) => forward(&socket, &buf[..len], to, "to_tap").await,
                    Decision::Lookup => {
                        hold(&mut pending, cfg.max_pending, id, &buf[..len], from);
                        spawn_lookup(Arc::clone(&routes), resolved_tx.clone(), id);
                    }
                    Decision::Pending => {
                        hold(&mut pending, cfg.max_pending, id, &buf[..len], from);
                    }
                    Decision::Drop(reason) => {
                        metrics::DROPPED.with_label_values(&[label(reason)]).inc();
                    }
                }
            }
            Some(r) = resolved_rx.recv() => {
                let now = Instant::now();
                table.resolved(r.id, r.sink, now);
                // Replayed through `route` rather than sent straight on, so the
                // held datagrams do the address pinning in the order they
                // actually arrived.
                let mut held = Vec::new();
                pending.retain(|p| {
                    if p.id == r.id {
                        held.push((p.datagram.clone(), p.from));
                        false
                    } else {
                        true
                    }
                });
                for (datagram, from) in held {
                    match table.route(r.id, from, now) {
                        Decision::ToSink(to) => forward(&socket, &datagram, to, "to_sink").await,
                        Decision::ToTap(to) => forward(&socket, &datagram, to, "to_tap").await,
                        Decision::Drop(reason) => {
                            metrics::DROPPED.with_label_values(&[label(reason)]).inc();
                        }
                        Decision::Lookup | Decision::Pending => {
                            metrics::DROPPED.with_label_values(&["resolving"]).inc();
                        }
                    }
                }
                metrics::ROUTES.set(table.live_routes() as i64);
            }
            _ = sweeper.tick() => {
                let now = Instant::now();
                // Anything still held when its lookup has been swept has no
                // route coming, so it is only holding memory.
                let idle = cfg.route_idle;
                pending.retain(|p| now.duration_since(p.since) < idle);
                let swept = table.sweep(now);
                if swept.idle > 0 {
                    metrics::EVICTED.with_label_values(&["idle"]).inc_by(swept.idle as u64);
                }
                if swept.negative > 0 {
                    metrics::EVICTED
                        .with_label_values(&["negative_expired"])
                        .inc_by(swept.negative as u64);
                }
                metrics::ROUTES.set(table.live_routes() as i64);
            }
        }
    }
}

struct Pending {
    id: protofish4::RequestId,
    datagram: Vec<u8>,
    from: SocketAddr,
    since: Instant,
}

/// Datagrams one request id may hold while its route resolves.
///
/// The global budget bounds memory; this bounds *fairness*. Without it a single
/// id spraying while its lookup is in flight fills the whole budget and denies
/// every other transfer its opening window — and the ids are attacker-chosen,
/// so that is a cheap thing to do on purpose.
const MAX_PENDING_PER_ID: usize = 8;

fn hold(
    pending: &mut VecDeque<Pending>,
    cap: usize,
    id: protofish4::RequestId,
    datagram: &[u8],
    from: SocketAddr,
) {
    if pending.len() >= cap {
        metrics::DROPPED.with_label_values(&["pending_full"]).inc();
        return;
    }
    // Over its allowance, an id drops its *oldest* held datagram rather than
    // refusing the new one. Which end gets dropped decides whether the transfer
    // survives: a missing `Data` is a gap the receiver NACKs back, but the
    // sender never retransmits `End`, so dropping the tail hangs the transfer
    // until its finalize timeout and fails it outright.
    if pending.iter().filter(|p| p.id == id).count() >= MAX_PENDING_PER_ID {
        if let Some(oldest) = pending.iter().position(|p| p.id == id) {
            pending.remove(oldest);
        }
        metrics::DROPPED.with_label_values(&["pending_per_id"]).inc();
    }
    pending.push_back(Pending {
        id,
        datagram: datagram.to_vec(),
        from,
        since: Instant::now(),
    });
}

async fn forward(socket: &UdpSocket, datagram: &[u8], to: SocketAddr, direction: &str) {
    if let Err(e) = socket.send_to(datagram, to).await {
        tracing::warn!(%e, %to, "failed to relay a datagram");
        metrics::DROPPED.with_label_values(&["send_failed"]).inc();
    } else {
        metrics::FORWARDED.with_label_values(&[direction]).inc();
    }
}

fn spawn_lookup(
    routes: Arc<dyn RouteSource>,
    tx: mpsc::Sender<Resolution>,
    id: protofish4::RequestId,
) {
    tokio::spawn(async move {
        let authority = routes.lookup(&id.0).await;
        // Resolved once, here, and never on the datagram path: the registry
        // stores authority strings, and a DNS round-trip per packet would be a
        // far worse cost than the Redis read it follows.
        let sink = match &authority {
            Some(a) => match lookup_host(a.as_str()).await {
                Ok(mut addrs) => addrs.next(),
                Err(e) => {
                    tracing::warn!(%e, %a, "could not resolve a sink address");
                    None
                }
            },
            None => None,
        };
        metrics::LOOKUPS
            .with_label_values(&[if sink.is_some() { "hit" } else { "miss" }])
            .inc();
        let _ = tx.send(Resolution { id, sink }).await;
    });
}

fn label(reason: DropReason) -> &'static str {
    match reason {
        DropReason::NoRoute => "no_route",
        DropReason::NotAParty => "not_a_party",
        DropReason::NoTapYet => "no_tap_yet",
        DropReason::Full => "table_full",
    }
}
