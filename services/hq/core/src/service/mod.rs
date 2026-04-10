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
pub use tap::TapService;
pub mod verification;
pub use verification::VerificationService;
pub mod user_settings;
pub use user_settings::UserSettingsService;
pub mod playback;
pub use mapping::MappingService;
pub use playback::{PlaybackService, UserVoiceInfo};
pub mod audio_engine;
pub use audio_engine::AudioEngineService;

use crate::repo::{
    PgApiKeyRepository, PgAuditLogRepo, PgGlobalSettingsRepository, PgGuildSettingsRepository,
    PgPlaybackActionRepo, PgTapRepository, PgTtsChannelRepo, PgUserGuildSettingsRepository,
    PgUserRepository,
};
use crate::{AppConfig, CoreError, CoreResult};
use sqlx::PgPool;
use std::sync::Arc;
use zako3_tl_client::TlClient;
use zako3_states::{
    IntendedVoiceChannelService, TapHubStateService, TapMetricsStateService,
    UserSettingsStateService, VoiceStateService,
};

#[derive(Clone)]
pub struct Service {
    pub config: Arc<AppConfig>,
    pub auth: AuthService,
    pub tap: TapService,
    pub notification: NotificationService,
    pub api_key: ApiKeyService,
    pub audit_log: AuditLogService,
    pub tap_metrics: TapMetricsStateService,
    pub verification: VerificationService,
    pub user_settings: UserSettingsService,
    pub voice_state: VoiceStateService,
    pub intended_vc: IntendedVoiceChannelService,
    pub playback: PlaybackService,
    pub mapping: MappingService,
    pub name_resolver_slot: DiscordNameResolverSlot,
    pub tts_channel: TTSChannelService,
    pub audio_engine: AudioEngineService,
}

impl Service {
    pub async fn new(pool: PgPool, timescale_pool: PgPool, config: Arc<AppConfig>) -> CoreResult<Self> {
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
        let tap_metrics_service = TapMetricsStateService::new(redis_repo.clone());
        let tap_hub_state_service = TapHubStateService::new(redis_repo.clone());
        let user_settings_cache = UserSettingsStateService::new(redis_repo.clone());

        let tap_service = TapService::new(
            timescale_pool,
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

        let audio_engine_service = AudioEngineService::new(audio_engine.clone());

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
            config.mapper_wasm_dir.clone(),
            config.mapper_db_path.clone(),
            name_resolver_slot.clone(),
        )
        .await?;

        let guild_settings_repo = Arc::new(PgGuildSettingsRepository::new(pool.clone()));
        let user_guild_settings_repo = Arc::new(PgUserGuildSettingsRepository::new(pool.clone()));
        let global_settings_repo = Arc::new(PgGlobalSettingsRepository::new(pool.clone()));

        Ok(Self {
            config: config.clone(),
            auth: AuthService::new(config.clone(), user_repo.clone()),
            tap: tap_service,
            api_key: api_key_service,
            notification: notification_service,
            audit_log: audit_log_service,
            tap_metrics: tap_metrics_service,
            verification: verification_service,
            user_settings: UserSettingsService::new(
                user_repo.clone(),
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
        })
    }
}
pub mod notification;
pub use notification::*;
