//! pf3 tap-side implementation. Uses the protofish3 chan/xfer API and the
//! [`crate::tap::TapHandler`] trait.

use hickory_resolver::{
    TokioAsyncResolver,
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    name_server::TokioConnectionProvider,
};
use protofish3::{
    ChanReceiver, ChanSender, Client, ClientConfig, ReconnectConfig, ReconnectingClient, XferMode,
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::error::{Result, ZakofishError};
use crate::tap::TapHandler;
use crate::tap_streams::encode_pf3_chunk;
use crate::types::TransferMode;
use crate::types::message::{HubToTapMessage, TapClientHello, TapDraining, TapToHubMessage};

/// What to do when the process is asked to stop.
///
/// The point of a drain is that a tap going away says so *before* it stops
/// answering. The alternative — dying on SIGTERM with the connection open —
/// leaves the hub with a QUIC connection that only fails on its idle timeout,
/// so requests keep being routed to a process that is already gone and each one
/// waits out its whole budget before the caller learns there was nobody there.
#[derive(Debug, Clone)]
pub struct DrainConfig {
    /// How long to keep answering requests that were already accepted before
    /// giving up and closing anyway. The process's own grace period has to be
    /// longer than this, or the kubelet kills it mid-drain.
    pub timeout: Duration,
    /// Whether to hook SIGTERM. On by default; off for tests and for anything
    /// that wants to drive the drain itself (see [`ZakofishTapPf3::drain_now`]).
    pub handle_signals: bool,
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            // Above the slowest single request any tap has been measured
            // serving (supertonic's synthesis is a 753ms worst case; youtube's
            // download is bounded by its own timeout), and well inside the
            // 45s grace period the manifests give the pod.
            timeout: Duration::from_secs(20),
            handle_signals: true,
        }
    }
}

pub struct ZakofishTapPf3 {
    client: Arc<Client>,
    /// Requests currently being served, and whether a drain has begun.
    ///
    /// Shared with every spawned per-request task so a drain can wait for them
    /// and so a new request arriving during the drain can be refused instead of
    /// started.
    in_flight: Arc<AtomicU32>,
    draining: Arc<AtomicBool>,
}

impl ZakofishTapPf3 {
    pub fn new(client_config: ClientConfig) -> Result<Self> {
        let client = Client::bind(client_config)?;
        Ok(Self {
            client: Arc::new(client),
            in_flight: Arc::new(AtomicU32::new(0)),
            draining: Arc::new(AtomicBool::new(false)),
        })
    }

    /// How many requests this tap is serving right now.
    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Begin draining without a signal, for a caller that owns the shutdown.
    ///
    /// Idempotent. A running [`Self::connect_and_run`] notices this within a
    /// few tens of milliseconds and drains the same way SIGTERM would.
    pub fn drain_now(&self) {
        self.draining.store(true, Ordering::SeqCst);
    }

    /// Connect to the hub and serve requests forever, reconnecting on failure.
    ///
    /// `hub_host` is a `"<host>:<port>"` string. DNS is re-resolved on an
    /// independent periodic timer so that IP changes due to DNS failover are
    /// detected even while the current [`ReconnectingClient`] is stuck retrying
    /// a permanently-dead address.
    ///
    /// ## Reconnect model
    ///
    /// Two layers of reconnect run concurrently inside this function:
    ///
    /// - **Inner** – [`ReconnectingClient`] handles transient failures (blips,
    ///   server restarts) against the *current* resolved IP with exponential
    ///   backoff.  When it succeeds, `reconnect_rx` signals and we re-handshake.
    ///
    /// - **Outer** – A DNS poll timer fires every [`DNS_POLL_INTERVAL`] and
    ///   re-resolves the hostname independently of the connection state.  If the
    ///   IP has changed (DNS failover), the old [`ReconnectingClient`] is dropped
    ///   (which cancels its retry loop) and the outer loop creates a new one
    ///   pointing at the new address.  This is the only path that can escape a
    ///   permanently-dead IP.
    pub async fn connect_and_run(
        &self,
        hub_host: &str,
        server_name: &str,
        hello_info: TapClientHello,
        handler: Arc<dyn TapHandler>,
    ) -> Result<()> {
        self.connect_and_run_with(
            hub_host,
            server_name,
            hello_info,
            handler,
            DrainConfig::default(),
        )
        .await
    }

    /// [`Self::connect_and_run`] with the shutdown behaviour spelled out.
    pub async fn connect_and_run_with(
        &self,
        hub_host: &str,
        server_name: &str,
        hello_info: TapClientHello,
        handler: Arc<dyn TapHandler>,
        drain: DrainConfig,
    ) -> Result<()> {
        /// How often to poll DNS regardless of connection state.
        const DNS_POLL_INTERVAL: Duration = Duration::from_secs(30);

        let mut shutdown = install_shutdown_signal(drain.handle_signals)?;

        let recon_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(8),
            backoff_multiplier: 2.0,
            max_retries: None,
            pool_size: 1,
        };

        // Build a resolver that queries 1.1.1.1 directly, bypassing the system
        // resolver.  Created once per `connect_and_run` invocation and reused
        // for all DNS polls.
        let resolver = {
            let mut cfg = ResolverConfig::new();
            cfg.add_name_server(NameServerConfig {
                socket_addr: (IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53).into(),
                protocol: Protocol::Udp,
                tls_dns_name: None,
                trust_negative_responses: false,
                bind_addr: None,
            });
            cfg.add_name_server(NameServerConfig {
                socket_addr: (IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)), 53).into(),
                protocol: Protocol::Udp,
                tls_dns_name: None,
                trust_negative_responses: false,
                bind_addr: None,
            });
            TokioAsyncResolver::new(
                cfg,
                ResolverOpts::default(),
                TokioConnectionProvider::default(),
            )
        };

        let mut current_addr = resolve(hub_host, &resolver).await?;

        'outer: loop {
            let conn = match ReconnectingClient::connect(
                self.client.clone(),
                current_addr,
                server_name.to_string(),
                HashMap::new(),
                recon_config.clone(),
            )
            .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(
                        "Failed to open ReconnectingClient to {}: {:?}; re-resolving and retrying",
                        current_addr,
                        e,
                    );
                    tokio::time::sleep(recon_config.initial_backoff).await;
                    if let Ok(addr) = resolve(hub_host, &resolver).await {
                        current_addr = addr;
                    }
                    continue 'outer;
                }
            };

            let mut reconnect_rx = conn.subscribe_reconnect();

            // DNS poll timer – fires independently of the connection state so
            // that a permanently-dead IP is detected even while
            // `ReconnectingClient` is stuck retrying it.
            let mut dns_interval = tokio::time::interval(DNS_POLL_INTERVAL);
            // Consume the first (immediate) tick so the select arm doesn't
            // fire right after a fresh connect.
            dns_interval.tick().await;

            if let Err(e) = do_handshake(&conn, &hello_info).await {
                tracing::error!(
                    "Initial handshake with {} failed: {:?}; retrying",
                    current_addr,
                    e,
                );
                tokio::time::sleep(recon_config.initial_backoff).await;
                continue 'outer;
            }

            // The chan the tap speaks on of its own accord. It carries exactly
            // one message, the drain notice, and it has to exist *before* the
            // drain so the notice does not wait on a handshake that is about to
            // stop being served. Best effort: without it the hub still learns
            // the tap left, just later and without the reason.
            let control = match conn.open_chan().await {
                Ok((sender, _receiver)) => Some(sender),
                Err(e) => {
                    tracing::warn!(%e, "no control chan; a drain notice will not reach the hub");
                    None
                }
            };

            loop {
                tokio::select! {
                    // ── Independent DNS poll ────────────────────────────────
                    // This arm fires every DNS_POLL_INTERVAL regardless of
                    // whether the underlying connection is healthy, retrying,
                    // or stuck against a dead IP.  It is the only mechanism
                    // that can detect a permanent IP change.
                    _ = dns_interval.tick() => {
                        match resolve(hub_host, &resolver).await {
                            Ok(new_addr) if new_addr != current_addr => {
                                tracing::info!(
                                    "DNS failover detected for '{}': {} → {}; \
                                     dropping current client and reconnecting",
                                    hub_host, current_addr, new_addr,
                                );
                                current_addr = new_addr;
                                // Dropping `conn` here cancels the
                                // ReconnectingClient's internal retry loop
                                // against the dead IP.
                                break; // → outer loop creates new client
                            }
                            Ok(_) => {
                                tracing::debug!(
                                    "DNS poll: '{}' still resolves to {} (unchanged)",
                                    hub_host, current_addr,
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "DNS poll for '{}' failed: {:?}; \
                                     keeping current address {}",
                                    hub_host, e, current_addr,
                                );
                            }
                        }
                    }

                    // ── Transient reconnect (same IP) ───────────────────────
                    // ReconnectingClient successfully reconnected to the same
                    // IP after a blip.  Re-handshake; no IP change.
                    _ = reconnect_rx.changed() => {
                        reconnect_rx.borrow_and_update();
                        tracing::info!(
                            "Reconnected to Hub at {} (same IP), re-sending ClientHello",
                            current_addr,
                        );
                        if let Err(e) = do_handshake(&conn, &hello_info).await {
                            tracing::error!("Re-handshake failed: {:?}", e);
                        }
                    }

                    // ── Incoming request channel ────────────────────────────
                    chan_result = conn.accept_chan() => {
                        match chan_result {
                            Ok((sender, receiver)) => {
                                let handler_clone = handler.clone();
                                let in_flight = Arc::clone(&self.in_flight);
                                let draining = Arc::clone(&self.draining);
                                tokio::spawn(async move {
                                    if let Err(e) = handle_incoming_chan(
                                        sender,
                                        receiver,
                                        handler_clone,
                                        in_flight,
                                        draining,
                                    )
                                    .await
                                    {
                                        tracing::error!("Error handling incoming chan: {:?}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!(
                                    "accept_chan error (addr={}): {:?}; re-resolving and reconnecting",
                                    current_addr, e,
                                );
                                if let Ok(addr) = resolve(hub_host, &resolver).await {
                                    current_addr = addr;
                                }
                                break; // → outer loop creates new client
                            }
                        }
                    }

                    // ── Shutdown ────────────────────────────────────────────
                    // SIGTERM (or an explicit `drain_now`) only sets the flag;
                    // the arm below acts on it, so a pod cancellation and a
                    // caller-driven drain take exactly the same path.
                    _ = wait_for_shutdown(&mut shutdown) => {
                        if !self.draining.swap(true, Ordering::SeqCst) {
                            tracing::info!("shutdown requested; draining");
                        }
                    }

                    // ── Drain ───────────────────────────────────────────────
                    // A plain timer rather than a guard on the flag: `select!`
                    // evaluates its guards once, when it is entered, and this
                    // one is entered on an idle connection that may then wait
                    // forever — so a guarded arm would never become live. 50ms
                    // is the ceiling on how long "shutdown requested" takes to
                    // become "notice on the wire", and the tick costs a timer
                    // wake that a tap's own loop would take anyway.
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        if !self.draining.load(Ordering::SeqCst) {
                            continue;
                        }
                        drain_and_close(
                            &conn,
                            control.as_ref(),
                            &self.in_flight,
                            &self.draining,
                            drain.timeout,
                        )
                        .await;
                        break 'outer;
                    }
                }
            }
            // `conn` is dropped here, cancelling any in-progress retry loop.
        }

        Ok(())
    }
}

/// The shutdown signal a tap listens for, where the platform has one.
#[cfg(unix)]
type ShutdownSignal = tokio::signal::unix::Signal;
/// Nothing to listen for off unix.
#[cfg(not(unix))]
struct ShutdownSignal;

fn install_shutdown_signal(wanted: bool) -> Result<Option<ShutdownSignal>> {
    if !wanted {
        return Ok(None);
    }

    #[cfg(unix)]
    {
        let sig = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| {
                ZakofishError::ProtocolError(format!("could not install SIGTERM handler: {e}"))
            })?;
        Ok(Some(sig))
    }

    #[cfg(not(unix))]
    {
        Ok(None)
    }
}

async fn wait_for_shutdown(sig: &mut Option<ShutdownSignal>) {
    match sig {
        #[cfg(unix)]
        Some(s) => {
            s.recv().await;
        }
        #[cfg(not(unix))]
        Some(_) => std::future::pending::<()>().await,
        None => std::future::pending::<()>().await,
    }
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Tell the hub, finish what is already running, then leave.
//
// The order is the whole design: the notice goes first so the hub stops handing
// this connection new requests while it is still able to answer, the in-flight
// wait keeps already-started work from being thrown away, and only then does the
// connection close — cleanly, with a close frame, so the hub sees a departure
// rather than a timeout.
//
// Both waits below are best-effort, not proofs: protofish3 acks neither a chan
// message nor a close.

/// Time to let the drain notice reach the hub before the connection is closed.
///
/// One frame over an established connection on a cluster network; the point is
/// only that it leaves the send buffer before CONNECTION_CLOSE does.
const NOTICE_SETTLE: Duration = Duration::from_millis(150);

/// Time to let CONNECTION_CLOSE itself leave the socket.
const CLOSE_SETTLE: Duration = Duration::from_millis(100);

async fn drain_and_close(
    conn: &ReconnectingClient,
    control: Option<&ChanSender>,
    in_flight: &Arc<AtomicU32>,
    draining: &Arc<AtomicBool>,
    timeout: Duration,
) {
    let started = Instant::now();
    let started_at_unix_ms = unix_millis();
    // Set before the notice is sent: a request that arrives between the two must
    // already be refused.
    draining.store(true, Ordering::SeqCst);
    let inflight = in_flight.load(Ordering::SeqCst);

    tracing::info!(
        in_flight = inflight,
        timeout_ms = timeout.as_millis() as u64,
        "draining: refusing new requests and telling the hub"
    );

    if let Some(control) = control {
        let notice = TapToHubMessage::Draining(TapDraining {
            in_flight: inflight,
            started_at_unix_ms,
        });
        let send = async {
            let bytes = crate::protocol::codec::encode_msgpack(&notice).ok()?;
            control.send_msg(bytes.to_vec()).await.ok()
        };
        // One second is generous for a single frame on a connection that is
        // still up, and short enough that a hub which has already gone does not
        // hold the drain up.
        if tokio::time::timeout(Duration::from_secs(1), send)
            .await
            .is_err()
        {
            tracing::warn!("drain notice did not reach the hub in 1s");
        }
    }

    while in_flight.load(Ordering::SeqCst) > 0 && started.elapsed() < timeout {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // The notice is written but not acknowledged — protofish3 has no ack for a
    // chan message — and closing the connection immediately would send
    // CONNECTION_CLOSE on top of stream data that has not left the send buffer
    // yet. The hub would then see a tap that vanished without saying anything,
    // which is the case this whole path exists to replace.
    tokio::time::sleep(NOTICE_SETTLE).await;

    let left = in_flight.load(Ordering::SeqCst);
    if left > 0 {
        tracing::warn!(
            left,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "drain timeout with requests still running; closing anyway"
        );
    } else {
        tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "drained");
    }

    conn.close(0).await;
    // `close` queues a CONNECTION_CLOSE on the driver's side; exiting
    // immediately can leave it unsent, and the hub would then be back to waiting
    // for an idle timeout on a socket nobody is holding.
    tokio::time::sleep(CLOSE_SETTLE).await;
}

/// Counts a request as in flight for as long as it is alive, however it ends.
struct InFlightGuard(Arc<AtomicU32>);

impl InFlightGuard {
    fn new(counter: Arc<AtomicU32>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Resolve a `"<host>:<port>"` string to a [`SocketAddr`] by querying
/// Cloudflare 1.1.1.1 directly, bypassing the OS system resolver.
async fn resolve(host: &str, resolver: &TokioAsyncResolver) -> Result<SocketAddr> {
    let (hostname, port_str) = host
        .rsplit_once(':')
        .ok_or_else(|| ZakofishError::ProtocolError(format!("invalid host:port '{}'", host)))?;
    let port: u16 = port_str
        .parse()
        .map_err(|_| ZakofishError::ProtocolError(format!("invalid port in '{}'", host)))?;

    let response = resolver.lookup_ip(hostname).await.map_err(|e| {
        ZakofishError::ProtocolError(format!(
            "DNS lookup failed for '{}' via 1.1.1.1: {}",
            hostname, e
        ))
    })?;

    let ip = response.iter().next().ok_or_else(|| {
        ZakofishError::ProtocolError(format!("no addresses found for '{}'", hostname))
    })?;

    Ok(SocketAddr::new(ip, port))
}

async fn do_handshake(conn: &ReconnectingClient, hello_info: &TapClientHello) -> Result<()> {
    let (sender, mut receiver) = conn.take_handshake_chan().await?;

    let hello_msg = TapToHubMessage::ClientHello(hello_info.clone());
    let encoded = crate::protocol::codec::encode_msgpack(&hello_msg)?;
    sender.send_msg(encoded.to_vec()).await?;

    let response_bytes = receiver.recv_msg().await?;
    let response: HubToTapMessage = crate::protocol::codec::decode_msgpack(&response_bytes)?;

    match response {
        HubToTapMessage::Accept => {
            tracing::info!("Tap connected and accepted by Hub.");
            Ok(())
        }
        HubToTapMessage::Reject(reject) => Err(ZakofishError::ProtocolError(format!(
            "Hub rejected connection: {:?}",
            reject
        ))),
        _ => Err(ZakofishError::ProtocolError(
            "Expected Accept or Reject".to_string(),
        )),
    }
}

async fn handle_incoming_chan(
    sender: ChanSender,
    mut receiver: ChanReceiver,
    handler: Arc<dyn TapHandler>,
    in_flight: Arc<AtomicU32>,
    draining: Arc<AtomicBool>,
) -> Result<()> {
    let payload_bytes = receiver.recv_msg().await?;
    let msg: HubToTapMessage = crate::protocol::codec::decode_msgpack(&payload_bytes)?;

    // A request that arrives after the drain started is refused rather than
    // served: this process is about to stop existing, so starting new work here
    // would only delay the hand-off to a tap that can actually finish it.
    if draining.load(Ordering::SeqCst)
        && matches!(
            &msg,
            HubToTapMessage::AudioRequest(_) | HubToTapMessage::AudioMetadataRequest(_)
        )
    {
        tracing::info!("refused a request that arrived while draining");
        let refusal = TapToHubMessage::AudioRequestFailure(
            crate::types::message::AudioRequestFailureMessage {
                reason: "tap is draining".to_string(),
                try_others: true,
            },
        );
        sender
            .send_msg(crate::protocol::codec::encode_msgpack(&refusal)?.to_vec())
            .await?;
        return Ok(());
    }

    // Held for the whole of this request: the drain waits on this count.
    let _in_flight = InFlightGuard::new(in_flight);

    match msg {
        HubToTapMessage::AudioRequest(request) => {
            match handler
                .handle_audio_request(request.ars, request.headers)
                .await
            {
                Ok((success_msg, mut chunk_receiver, transfer_mode)) => {
                    let response_msg = TapToHubMessage::AudioRequestSuccess(success_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;

                    let mut send_xfer = sender.start_xfer(map_mode(transfer_mode)).await?;

                    while let Some((timestamp, bytes)) = chunk_receiver.recv().await {
                        tracing::trace!(
                            "Sending pf3 chunk timestamp={} size={}",
                            timestamp.0,
                            bytes.len()
                        );
                        let buf = encode_pf3_chunk(timestamp, &bytes);
                        send_xfer.send(buf).await?;
                    }

                    send_xfer.end().await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;
                }
            }
        }
        HubToTapMessage::AudioMetadataRequest(request) => {
            match handler
                .handle_audio_metadata_request(request.ars, request.headers)
                .await
            {
                Ok(success_msg) => {
                    let response_msg = TapToHubMessage::AudioMetadataSuccess(success_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;
                }
            }
        }
        _ => {
            tracing::warn!("Received unexpected message on pf3 data chan: {:?}", msg);
        }
    }

    Ok(())
}

fn map_mode(mode: TransferMode) -> XferMode {
    match mode {
        TransferMode::Dual => XferMode::Dual,
        TransferMode::UnreliableOnly => XferMode::Unrel,
    }
}
