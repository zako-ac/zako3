pub mod auth;
pub mod discord_resolver;
pub mod mapping;
pub mod tap;
pub mod tts_channel;
pub mod validation;

pub use auth::AuthService;
pub use discord_resolver::{
    DiscordNameResolver, DiscordNameResolverSlot, DiscordUserInfo, GuildInfo, make_resolver_slot,
};
pub use tts_channel::TTSChannelService;
pub mod api_key;
pub use api_key::ApiKeyService;
pub mod audit_log;
pub use audit_log::AuditLogService;
pub use auth::Claims; // Export Claims
pub use tap::{SortDirection, TapService, TapSortField};
pub mod verification;
pub use verification::VerificationService;
pub mod user_settings;
pub use user_settings::UserSettingsService;
pub mod playback;
pub use mapping::MappingService;
pub use playback::{PlaybackService, UserVoiceInfo};
pub mod audio_engine;
pub use audio_engine::AudioEngineService;
pub mod emoji_match_publisher;
pub use emoji_match_publisher::EmojiMatchPublisher;

use crate::repo::{
    PgApiKeyRepository, PgAuditLogRepo, PgGlobalSettingsRepository, PgGuildSettingsRepository,
    PgPlaybackActionRepo, PgTapRepository, PgTtsChannelRepo,
    PgUserGuildSettingsRepository, PgUserRepository,
};
use crate::{AppConfig, CoreError, CoreResult};
use hq_types::hq::playback::PlaybackEvent;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use zako3_cache_client::RemoteAudioCache;
use zako3_metrics::TapMetricsService;
use zako3_tl_client::TlClient;
use zako3_states::{
    IntendedVoiceChannelService, TapHubStateService, UserSettingsStateService, VoiceStateService,
};

#[derive(Clone)]
pub struct Service {
    pub config: Arc<AppConfig>,
    pub auth: AuthService,
    pub tap: TapService,
    pub notification: NotificationService,
    pub api_key: ApiKeyService,
    pub audit_log: AuditLogService,
    pub tap_metrics: TapMetricsService,
    pub verification: VerificationService,
    pub user_settings: UserSettingsService,
    pub voice_state: VoiceStateService,
    pub intended_vc: IntendedVoiceChannelService,
    pub playback: PlaybackService,
    pub mapping: MappingService,
    pub name_resolver_slot: DiscordNameResolverSlot,
    pub tts_channel: TTSChannelService,
    pub audio_engine: AudioEngineService,
    pub emoji_match_publisher: Option<EmojiMatchPublisher>,
    /// Admin client for the cache worker (clear/delete cached audio).
    pub cache_admin: Arc<RemoteAudioCache>,
}

impl Service {
    pub async fn new(pool: PgPool, timescale_pool: Option<PgPool>, config: Arc<AppConfig>, event_tx: broadcast::Sender<PlaybackEvent>) -> CoreResult<Self> {
        let user_repo = Arc::new(PgUserRepository::new(pool.clone()));
        let tap_repo = Arc::new(PgTapRepository::new(pool.clone()));
        let api_key_repo = Arc::new(PgApiKeyRepository::new(pool.clone()));
        let audit_log_repo = Arc::new(PgAuditLogRepo::new(pool.clone()));
        let verification_repo = Arc::new(crate::repo::PgVerificationRepository::new(pool.clone()));
        let tts_channel_repo = Arc::new(PgTtsChannelRepo::new(pool.clone()));

        let audit_log_service = AuditLogService::new(audit_log_repo.clone());
        let notification_repo = Arc::new(crate::repo::PgNotificationRepository::new(pool.clone()));
        let notification_service = NotificationService::new(notification_repo);

        let redis_url = &config.redis_url;
        let redis_repo = Arc::new(zako3_states::RedisCacheRepository::new(redis_url).await?);

        let tap_metrics_service = TapMetricsService::new(redis_repo.clone(), timescale_pool, Some(pool.clone()));

        // Spawn history subscriber background task
        let pubsub = zako3_states::RedisPubSub::new(redis_url)
            .await
            .map_err(|e| CoreError::Internal(format!("Redis pubsub error: {e}")))?;
        // Shared handle for mapper-cache publish (writes) and subscribe (invalidation).
        let mapper_pubsub = Arc::new(pubsub.clone());
        let history_pubsub = pubsub;
        let history_metrics = tap_metrics_service.clone();
        let history_redis_url = redis_url.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut history_pubsub = Some(history_pubsub);
            loop {
                let pubsub = match history_pubsub.take() {
                    Some(p) => Ok(p),
                    None => zako3_states::RedisPubSub::new(&history_redis_url)
                        .await
                        .map_err(|e| CoreError::Internal(format!("Redis pubsub error: {e}"))),
                };
                match pubsub {
                    Ok(pubsub) => match pubsub.subscribe_history().await {
                        Ok(stream) => {
                            let mut stream = Box::pin(stream);
                            while let Some(entry) = stream.next().await {
                                if let hq_types::hq::history::UseHistoryEntry::PlayAudio(ref h) =
                                    entry
                                {
                                    if let Err(e) = history_metrics.insert_history(h).await {
                                        tracing::warn!(%e, "Failed to insert use_history entry");
                                    }
                                }
                            }
                            tracing::warn!("history subscription ended; reconnecting");
                        }
                        Err(e) => tracing::error!(%e, "Failed to subscribe to history channel"),
                    },
                    Err(e) => tracing::error!(%e, "Failed to connect Redis pubsub for history"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        let tap_hub_state_service = TapHubStateService::new(redis_repo.clone());
        let user_settings_cache = UserSettingsStateService::new(redis_repo.clone());

        let tap_service = TapService::new(
            tap_repo.clone(),
            user_repo.clone(),
            audit_log_service.clone(),
            tap_metrics_service.clone(),
            tap_hub_state_service,
        );
        let api_key_service = ApiKeyService::new(
            api_key_repo.clone(),
            tap_repo.clone(),
            audit_log_service.clone(),
        );

        let verification_service = VerificationService::new(
            verification_repo,
            tap_repo.clone(),
            audit_log_service.clone(),
            notification_service.clone(),
        );

        let audio_engine = Arc::new(
            TlClient::connect(&config.traffic_light_url)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?,
        );

        let audio_engine_service = AudioEngineService::new(audio_engine.clone(), event_tx);

        let voice_state = VoiceStateService::new(redis_repo.clone());
        let intended_vc = IntendedVoiceChannelService::new(redis_repo.clone());
        let playback_action_repo = Arc::new(PgPlaybackActionRepo::new(pool.clone()));
        let name_resolver_slot = make_resolver_slot();
        let playback = PlaybackService::new(
            audio_engine_service.clone(),
            voice_state.clone(),
            playback_action_repo,
            name_resolver_slot.clone(),
        );

        let mapping = MappingService::new(
            pool.clone(),
            name_resolver_slot.clone(),
            Some(mapper_pubsub.clone()),
        )?;

        // Mapper-cache invalidation subscriber. Reconnect loop: refresh on every (re)connect
        // (re-syncing any events missed while disconnected), then reload on each event.
        let sub_pubsub = mapper_pubsub.clone();
        let sub_mapping = mapping.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            loop {
                // Warm / re-sync on connect before consuming the stream.
                sub_mapping.refresh_cache().await;
                match (*sub_pubsub).clone().subscribe_mapper_cache().await {
                    Ok(stream) => {
                        let mut stream = Box::pin(stream);
                        while let Some(event) = stream.next().await {
                            tracing::debug!(?event, "mapper-cache invalidation received");
                            sub_mapping.refresh_cache().await;
                        }
                        tracing::warn!("mapper-cache subscription ended; reconnecting");
                    }
                    Err(e) => {
                        tracing::error!(%e, "failed to subscribe to mapper-cache channel");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });

        // Periodic safety refresh — backstop for any missed (fire-and-forget) pub/sub message.
        let refresh_secs = config.mapper_cache_refresh_secs;
        if refresh_secs > 0 {
            let refresh_mapping = mapping.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(refresh_secs));
                interval.tick().await; // skip the immediate first tick (subscriber already warms)
                loop {
                    interval.tick().await;
                    refresh_mapping.refresh_cache().await;
                }
            });
        }

        let cache_admin = Arc::new(
            RemoteAudioCache::new(
                config.cache_rpc_url.clone(),
                config.cache_rpc_admin_token.clone(),
            )
            .map_err(|e| CoreError::Internal(format!("cache client error: {e}")))?,
        );

        let guild_settings_repo = Arc::new(PgGuildSettingsRepository::new(pool.clone()));
        let user_guild_settings_repo = Arc::new(PgUserGuildSettingsRepository::new(pool.clone()));
        let global_settings_repo = Arc::new(PgGlobalSettingsRepository::new(pool.clone()));

        let emoji_match_publisher = match config.nats_url.as_deref() {
            Some(url) => match EmojiMatchPublisher::connect(url).await {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to connect emoji-match publisher; continuing without it");
                    None
                }
            },
            None => {
                tracing::info!("NATS_URL not set; emoji-match publisher disabled");
                None
            }
        };

        Ok(Self {
            config: config.clone(),
            auth: AuthService::new(config.clone(), user_repo.clone(), redis_repo.clone()),
            tap: tap_service,
            api_key: api_key_service,
            notification: notification_service,
            audit_log: audit_log_service,
            tap_metrics: tap_metrics_service,
            verification: verification_service,
            user_settings: UserSettingsService::new(
                user_repo.clone(),
                tap_repo.clone(),
                guild_settings_repo,
                user_guild_settings_repo,
                global_settings_repo,
                user_settings_cache,
            ),
            voice_state,
            intended_vc,
            playback,
            mapping,
            name_resolver_slot,
            tts_channel: TTSChannelService::new(tts_channel_repo),
            audio_engine: audio_engine_service,
            emoji_match_publisher,
            cache_admin,
        })
    }
}
pub mod notification;
pub use notification::*;
