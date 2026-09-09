use std::sync::{Arc, OnceLock};
use std::time::Duration;
use std::net::SocketAddr;

use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::SerenityInit;
use jsonrpsee::server::Server;
use tl_protocol::AudioEngineRpcServer;

use zako3_audio_engine_controller::{
    address::{SelfAddressResolver, HeuristicSelfAddressResolver},
    config::AppConfig,
    guild_reporter::{report_guilds_once, run_ae_heartbeat, run_guild_reporter},
    ready_waiter::create_ready_waiter,
    server::AeTransportHandler,
};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_infra::{
    RedisStateService, discord::SongbirdDiscordService, taphub::RealTapHubService,
};

use zako3_telemetry::TelemetryConfig;
use zako3_tl_client::TlClient;
use zako3_taphub_transport_client::{TransportClient, load_certs};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();

    println!("Starting zako3 audio engine...");

    let _ = rustls::crypto::ring::default_provider().install_default();

    let telem_config = TelemetryConfig {
        service_name: config.service_name.clone(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: Some(config.metrics_port),
    };

    let telemetry = zako3_telemetry::init(telem_config).await?;

    let certs = load_certs(&config.taphub_transport_cert_file).unwrap_or_else(|_| vec![]);

    // Create shared lazy cell for taphub connection
    let taphub_cell: Arc<tokio::sync::Mutex<Option<Arc<TransportClient>>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    // Spawn background retry task
    {
        let cell = taphub_cell.clone();
        let config = config.clone();
        let certs = certs.clone();
        tokio::spawn(async move {
            loop {
                match TransportClient::connect(
                    "0.0.0.0:0".parse().unwrap(),
                    &config.taphub_url,
                    config.taphub_sni.clone(),
                    certs.clone(),
                    Duration::from_millis(config.taphub_request_timeout_ms),
                )
                .await
                {
                    Ok(t) => {
                        tracing::info!("Connected to taphub");
                        *cell.lock().await = Some(Arc::new(t));
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Taphub connect failed: {:?}. Retrying in 5s...", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    let legacy_taphub: Arc<dyn zako3_audio_engine_core::service::taphub::TapHubService> =
        Arc::new(RealTapHubService::new_lazy(taphub_cell));

    // The v4 audio plane, when configured. It wraps the taphub service rather
    // than replacing it: HQ answers `Legacy` for any tap that has not been
    // migrated, and the request falls straight through to the path above.
    let taphub_service = match build_hq_audio(&config, Arc::clone(&legacy_taphub)).await {
        Ok(Some(svc)) => {
            tracing::info!(sink_id = %config.sink_id(), "v4 audio plane enabled");
            svc
        }
        Ok(None) => {
            tracing::info!("HQ_RPC_URL not set; using the taphub path only");
            legacy_taphub
        }
        Err(e) => {
            // Falling back rather than failing to start: a broken v4 setup must
            // not take audio down when the legacy path is right there.
            tracing::error!(%e, "failed to start the v4 audio plane; using taphub only");
            legacy_taphub
        }
    };

    // Step 1: Resolve advertised address.
    let advertised_addr = if let Some(addr) = &config.ae_advertise_addr {
        // Use explicit override if provided
        addr.clone()
    } else {
        // Otherwise, use heuristic resolution
        let address_resolver = HeuristicSelfAddressResolver::new(config.ae_port);
        match address_resolver.resolve() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::error!(
                    "Failed to resolve advertised address: {}. \
                     Set AE_ADVERTISE_ADDR=host:port to override.",
                    e
                );
                std::process::exit(1);
            }
        }
    };

    tracing::info!("Advertised address: {}", advertised_addr);

    // Step 2: Create OnceLock for session_manager (will be filled later).
    let sm_cell: Arc<OnceLock<Arc<SessionManager>>> = Arc::new(OnceLock::new());

    // Step 3: Start jsonrpsee HTTP server for TL to call this AE (before registering).
    let ae_listen_addr: SocketAddr = format!("0.0.0.0:{}", config.ae_port)
        .parse()
        .expect("Invalid AE listen address");

    let handler = AeTransportHandler::new(sm_cell.clone());
    let server = Server::builder()
        .build(ae_listen_addr)
        .await
        .expect("Failed to bind AE HTTP server");

    tracing::info!("AE HTTP server listening on {}", ae_listen_addr);

    let server_handle = server.start(handler.into_rpc());

    // Step 4: Register with TL and receive Discord token.
    let tl_client = loop {
        match TlClient::connect(&config.tl_rpc_url).await {
            Ok(c) => break c,
            Err(e) => {
                tracing::warn!("Failed to connect to TL client: {e}, retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    let token_str = loop {
        match tl_client.register_ae(advertised_addr.clone()).await {
            Ok(t) => {
                tracing::info!("Registered with TL and received Discord token");
                break t;
            }
            Err(e) => {
                tracing::warn!("Failed to register with TL: {e}, retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    // Step 5: Spawn AE heartbeat to re-register with TL every 15 s (handles TL restarts).
    tokio::spawn(run_ae_heartbeat(
        config.tl_rpc_url.clone(),
        token_str.clone(),
        advertised_addr.clone(),
    ));

    // Step 6: Build the Discord / audio infrastructure with the assigned token.
    let songbird_manager = songbird::Songbird::serenity();
    let (ready_waiter, mut ready_recv, mut ctx_recv) = create_ready_waiter();

    let intents = GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILDS;
    let mut discord_client = Client::builder(&token_str, intents)
        .event_handler_arc(ready_waiter)
        .register_songbird_with(songbird_manager.clone())
        .await
        .expect("Failed to create Discord client");

    tokio::spawn(async move {
        if let Err(e) = discord_client.start().await {
            tracing::error!("Discord client ended: {:?}", e);
        } else {
            tracing::warn!("Discord client exited without error");
        }
        tracing::error!("Discord task terminated, shutting down AE process for restart");
        std::process::exit(1);
    });

    ready_recv.recv().await;
    let serenity_ctx = ctx_recv.recv().await.expect("ctx channel closed before ready");

    // Channel carrying terminal voice disconnects from the songbird driver-event watcher
    // (registered per-join) to the controller's consumer task below.
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::unbounded_channel::<(
        zako3_audio_engine_core::types::GuildId,
        zako3_audio_engine_core::types::ChannelId,
    )>();

    let discord_service = Arc::new(SongbirdDiscordService::new(
        songbird_manager.clone(),
        serenity_ctx.cache.clone(),
        disconnect_tx,
    ));

    // Redis-backed session store, namespaced by this bot's token so a restart rejoins exactly
    // this bot's previously-active channels. Best-effort: degrades to no persistence if Redis
    // is unavailable.
    let state_service = Arc::new(RedisStateService::new(&config.redis_url, &token_str).await);

    let session_manager = Arc::new(SessionManager::new(
        discord_service,
        state_service,
        taphub_service,
    ));

    // Fill the OnceLock now that session_manager is constructed
    let _ = sm_cell.set(session_manager.clone());

    // Rejoin any sessions persisted from a previous run. The bot reconnects to its channels
    // automatically; TL's reconcile will re-adopt these live sessions into its cache.
    match session_manager.list_sessions().await {
        Ok(persisted) if !persisted.is_empty() => {
            tracing::info!(count = persisted.len(), "Rejoining persisted voice sessions");
            for s in persisted {
                if let Err(e) = session_manager.rejoin(&s).await {
                    tracing::error!(
                        guild_id = %s.guild_id,
                        channel_id = %s.channel_id,
                        "rejoin failed; dropping persisted session: {e}"
                    );
                    let _ = session_manager
                        .cleanup_session(s.guild_id, s.channel_id)
                        .await;
                }
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("failed to list persisted sessions for rejoin: {e}"),
    }

    // Consume terminal voice disconnects (songbird exhausted reconnects). Clean up the dead
    // session so TL's cache stays consistent and reconcile won't see it as dangling.
    {
        let sm = session_manager.clone();
        tokio::spawn(async move {
            while let Some((guild_id, channel_id)) = disconnect_rx.recv().await {
                tracing::warn!(
                    ?guild_id,
                    ?channel_id,
                    "terminal voice disconnect; cleaning up session"
                );
                if let Err(e) = sm.cleanup_session(guild_id, channel_id).await {
                    tracing::error!(
                        ?guild_id,
                        ?channel_id,
                        "driver-disconnect cleanup failed: {e}"
                    );
                }
            }
        });
    }

    tracing::info!("Audio Engine is ready and connected to Discord!");
    telemetry.healthy();

    // Step 6: Spawn background guild reporter.
    tokio::spawn(run_guild_reporter(
        serenity_ctx.clone(),
        config.tl_rpc_url.clone(),
        token_str.clone(),
    ));

    // Report guilds once on startup
    report_guilds_once(&serenity_ctx, &config.tl_rpc_url, &token_str).await;

    tracing::info!("Audio Engine is serving requests");

    // Step 7: Wait for server to stop OR shutdown signal
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = server_handle.stopped() => {
                tracing::error!("AE HTTP server stopped unexpectedly");
                std::process::exit(1);
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT, shutting down Audio Engine");
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down Audio Engine");
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::select! {
            _ = server_handle.stopped() => {
                tracing::error!("AE HTTP server stopped unexpectedly");
                std::process::exit(1);
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received ctrl+c, shutting down Audio Engine");
            }
        }
    }

    Ok(())
}


/// Build the v4 audio plane, or `None` when HQ is not configured.
///
/// Binds the UDP socket, starts advertising this engine as a sink, and returns
/// a service that falls back to `legacy` for any tap HQ has not migrated.
async fn build_hq_audio(
    config: &AppConfig,
    legacy: Arc<dyn zako3_audio_engine_core::service::taphub::TapHubService>,
) -> anyhow::Result<Option<Arc<dyn zako3_audio_engine_core::service::taphub::TapHubService>>> {
    let Some(hq_url) = config.hq_rpc_url.clone() else {
        return Ok(None);
    };

    let hq = zako3_audio_engine_infra::hq_client::HqAudioClient::new(
        &hq_url,
        config.hq_rpc_admin_token.as_deref().unwrap_or_default(),
        Duration::from_millis(config.hq_request_timeout_ms),
    )?;

    let bind: std::net::SocketAddr = config.udp_bind_addr.parse()?;
    let endpoint = protofish4::Endpoint::bind(bind).await?;
    tracing::info!(addr = %endpoint.local_addr()?, "protofish4 ingest listening");

    // One reader for the socket, plus a ticker driving per-transfer timers —
    // NACK retries and stall detection both hang off that.
    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            if let Err(err) = e.run().await {
                tracing::error!(%err, "protofish4 endpoint stopped");
            }
        }
    });
    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(10));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                e.tick().await;
            }
        }
    });

    let cache = Arc::new(zako3_cache_client::RemoteAudioCache::new(
        config.cache_rpc_url.clone(),
        config.cache_rpc_admin_token.clone(),
    )?);

    // Advertise where taps should send. `public_addr` stays unset while the
    // shared proxy owns the only public IP; setting it later moves taps
    // straight here with no protocol change.
    let registry = zako3_states::SinkRegistry::new(Arc::new(
        zako3_states::RedisCacheRepository::new(&config.redis_url).await?,
    ));
    zako3_audio_engine_infra::hq_audio::spawn_sink_heartbeat(
        registry,
        zako3_states::SinkAdvertisement {
            sink_id: config.sink_id(),
            kind: zako3_states::SinkKind::AudioEngine,
            internal_addr: config.udp_internal_addr(),
            public_addr: config.udp_public_addr.clone(),
        },
    );

    Ok(Some(Arc::new(
        zako3_audio_engine_infra::hq_audio::HqAudioService::new(
            hq,
            endpoint,
            cache,
            config.sink_id(),
            legacy,
        ),
    )))
}
