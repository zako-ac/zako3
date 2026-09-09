//! The shared UDP proxy.
//!
//! Temporary by design. While one public IP is shared, taps are told to send
//! here and this routes each datagram to the sink that owns its request id.
//! Once every sink has its own IP, HQ puts the sink's address in `deliver_to`
//! and this worker is deleted — a pure removal, with no protocol change.
//!
//! The name is narrower than the job: preloads route to the *cache worker*
//! through here too, and the proxy cannot tell the difference. That is what
//! keeps its state a plain `request_id → SocketAddr` map. `udp-proxy` would be
//! the honest name; `ae-proxy` is the one it was given.

pub mod config;
pub mod http;
pub mod metrics;
pub mod relay;
pub mod table;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use config::Config;

pub async fn run() -> anyhow::Result<()> {
    let config = Config::from_env()?;

    let telemetry = zako3_telemetry::init(zako3_telemetry::TelemetryConfig {
        service_name: "ae-proxy".to_string(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: None,
    })
    .await?;

    http::spawn(&config.http_addr).await?;

    let socket = Arc::new(tokio::net::UdpSocket::bind(&config.bind_addr).await?);
    tracing::info!(addr = %socket.local_addr()?, "udp proxy listening");

    let registry = zako3_states::SinkRegistry::new(Arc::new(
        zako3_states::RedisCacheRepository::new(&config.redis_url).await?,
    ));

    http::IS_HEALTHY.store(true, Ordering::Relaxed);
    telemetry.healthy();

    tokio::select! {
        res = relay::run(
            Arc::clone(&socket),
            Arc::new(registry),
            relay::RelayConfig {
                max_routes: config.max_routes,
                max_negative: config.max_negative,
                max_pending: config.max_pending,
                route_idle: config.route_idle,
                negative_ttl: config.negative_ttl,
            },
        ) => {
            if let Err(e) = res {
                tracing::error!(%e, "udp proxy stopped");
                return Err(e);
            }
        }
        res = tokio::signal::ctrl_c() => {
            if let Err(e) = res {
                tracing::warn!(%e, "failed to listen for Ctrl-C");
            }
            tracing::info!("Ctrl-C received, shutting down");
        }
    }

    Ok(())
}
