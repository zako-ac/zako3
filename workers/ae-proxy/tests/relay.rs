//! A whole protofish4 transfer through the proxy, plus the two failure modes
//! that would otherwise only show up in production.
//!
//! The transfer is the real thing — a real sender, a real receiver, and the
//! relay loop between them on one socket — because what is being checked is
//! that the *return* path works. A test that only asserted frames arrive would
//! pass with every `Ack` and `Nack` in the bin.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::UdpSocket;
use zako3_ae_proxy::relay::{RelayConfig, RouteSource, run};

/// Stands in for Redis. Also counts lookups, which is how the negative-cache
/// behaviour is observable at all.
struct Routes {
    sink: Option<String>,
    lookups: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl RouteSource for Routes {
    async fn lookup(&self, _request_id: &uuid::Uuid) -> Option<String> {
        self.lookups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.sink.clone()
    }
}

struct Proxy {
    addr: SocketAddr,
    lookups: Arc<std::sync::atomic::AtomicUsize>,
}

async fn spawn_proxy(sink: Option<SocketAddr>) -> Proxy {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.expect("bind proxy"));
    let addr = socket.local_addr().expect("proxy addr");
    let lookups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let routes = Arc::new(Routes {
        sink: sink.map(|a| a.to_string()),
        lookups: Arc::clone(&lookups),
    });
    tokio::spawn(async move {
        let _ = run(
            socket,
            routes,
            RelayConfig {
                max_routes: 16,
                max_negative: 16,
                max_pending: 512,
                route_idle: Duration::from_secs(60),
                negative_ttl: Duration::from_secs(5),
            },
        )
        .await;
    });
    Proxy { addr, lookups }
}

fn frames(n: usize) -> Vec<(protofish4::TimestampMs, Vec<u8>)> {
    (0..n)
        .map(|i| {
            (
                protofish4::TimestampMs(i as u64 * 20),
                (0..60u8).map(|b| b.wrapping_add(i as u8)).collect(),
            )
        })
        .collect()
}

/// End to end: a tap that only knows the proxy's address delivers a complete,
/// gap-free transfer to a sink it never addresses directly.
///
/// The 20 frames are deliberately more than one id's pending allowance, and
/// `send_all` emits them as a burst — so the whole opening window lands while
/// the route is still resolving. That makes this the regression test for two
/// things at once: that the opening datagrams are held rather than dropped, and
/// that when an id must shed some of them it sheds the oldest. Shedding the
/// newest loses `End`, which the sender never retransmits, and the transfer
/// then hangs until its finalize timeout instead of NACK-recovering a gap.
#[tokio::test]
async fn a_transfer_completes_through_the_proxy() {
    let endpoint = protofish4::Endpoint::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind sink");
    let sink_addr = endpoint.local_addr().expect("sink addr");
    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            let _ = e.run().await;
        }
    });
    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            let mut t = tokio::time::interval(Duration::from_millis(10));
            loop {
                t.tick().await;
                e.tick().await;
            }
        }
    });

    let proxy = spawn_proxy(Some(sink_addr)).await;

    let raw = protofish4::random_key();
    let request_id = protofish4::RequestId(uuid::Uuid::new_v4());
    let (_armed, mut streams) = endpoint
        .arm(
            request_id,
            protofish4::SessionKey::from_bytes(&raw).unwrap(),
            protofish4::ReceiverConfig::cache_worker(),
        )
        .await
        .expect("arm");

    let sent = frames(20);
    let send = tokio::spawn({
        let (proxy_addr, sent) = (proxy.addr.to_string(), sent.clone());
        async move {
            protofish4::send_all(
                vec![proxy_addr],
                request_id,
                protofish4::SessionKey::from_bytes(&raw).unwrap(),
                protofish4::SenderConfig::default(),
                sent,
            )
            .await
        }
    });

    let mut got = Vec::new();
    while let Some(frame) = streams.rel.recv().await {
        got.push(frame.payload);
    }

    send.await.expect("join").expect("send through the proxy");
    let outcome = streams.outcome.await.expect("outcome");
    assert!(
        matches!(outcome, protofish4::RelOutcome::Complete { .. }),
        "the reliable stream must complete, which it only can if the sink's \
         acks found their way back through the proxy: {outcome:?}"
    );
    assert_eq!(
        got,
        sent.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>()
    );
}

/// A datagram for a request id with no route is dropped, and the proxy asks
/// about it once rather than on every packet.
#[tokio::test]
async fn an_unroutable_id_is_asked_about_once() {
    let proxy = spawn_proxy(None).await;
    let tap = UdpSocket::bind("127.0.0.1:0").await.expect("bind tap");

    let raw = protofish4::random_key();
    let key = protofish4::SessionKey::from_bytes(&raw).unwrap();
    let id = protofish4::RequestId(uuid::Uuid::new_v4());
    let header = protofish4::proto::header::Header::new(
        protofish4::proto::frame::PacketKind::Keepalive,
        id,
        0,
    );
    let datagram = protofish4::proto::crypto::seal(
        &key,
        header,
        protofish4::proto::types::Direction::Send,
        &[0u8; 4],
    )
    .expect("seal");

    for _ in 0..5 {
        tap.send_to(&datagram, proxy.addr).await.expect("send");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        proxy.lookups.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "a spray of packets for one unroutable id must not become a Redis load test"
    );
}

/// Anything that is not a protofish4 datagram is dropped without the proxy
/// making a routing decision about it.
#[tokio::test]
async fn junk_is_dropped_without_a_lookup() {
    let proxy = spawn_proxy(Some("127.0.0.1:9".parse().unwrap())).await;
    let tap = UdpSocket::bind("127.0.0.1:0").await.expect("bind tap");

    tap.send_to(b"not protofish4", proxy.addr).await.expect("send");
    tap.send_to(&[0u8; 40], proxy.addr).await.expect("send");
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(proxy.lookups.load(std::sync::atomic::Ordering::Relaxed), 0);
}
