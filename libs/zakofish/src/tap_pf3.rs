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
use std::time::Duration;

use crate::error::{Result, ZakofishError};
use crate::tap::TapHandler;
use crate::tap_streams::encode_pf3_chunk;
use crate::types::TransferMode;
use crate::types::message::{HubToTapMessage, TapClientHello, TapToHubMessage};

pub struct ZakofishTapPf3 {
    client: Arc<Client>,
}

impl ZakofishTapPf3 {
    pub fn new(client_config: ClientConfig) -> Result<Self> {
        let client = Client::bind(client_config)?;
        Ok(Self {
            client: Arc::new(client),
        })
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
        /// How often to poll DNS regardless of connection state.
        const DNS_POLL_INTERVAL: Duration = Duration::from_secs(30);

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
                                tokio::spawn(async move {
                                    if let Err(e) =
                                        handle_incoming_chan(sender, receiver, handler_clone).await
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
                }
            }
            // `conn` is dropped here, cancelling any in-progress retry loop.
        }
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
) -> Result<()> {
    let payload_bytes = receiver.recv_msg().await?;
    let msg: HubToTapMessage = crate::protocol::codec::decode_msgpack(&payload_bytes)?;

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
