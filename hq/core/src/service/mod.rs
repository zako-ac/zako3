pub mod auth;
pub mod tap;
pub mod validation;

pub use auth::AuthService;
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

use crate::repo::{PgApiKeyRepository, PgAuditLogRepo, PgTapRepository, PgUserRepository};
use crate::{AppConfig, CoreResult};
use sqlx::PgPool;
use std::sync::Arc;

use zako3_states::{TapMetricsStateService, UserSettingsStateService};

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
}

impl Service {
    pub async fn new(pool: PgPool, config: Arc<AppConfig>) -> CoreResult<Self> {
        let user_repo = Arc::new(PgUserRepository::new(pool.clone()));
        let tap_repo = Arc::new(PgTapRepository::new(pool.clone()));
        let api_key_repo = Arc::new(PgApiKeyRepository::new(pool.clone()));
        let audit_log_repo = Arc::new(PgAuditLogRepo::new(pool.clone()));
        let verification_repo = Arc::new(crate::repo::PgVerificationRepository::new(pool.clone()));

        let audit_log_service = AuditLogService::new(audit_log_repo.clone());
        let notification_repo = Arc::new(crate::repo::PgNotificationRepository::new(pool.clone()));
        let notification_service = NotificationService::new(notification_repo);

        let redis_url = &config.redis_url;
        let redis_repo = Arc::new(zako3_states::RedisCacheRepository::new(redis_url).await?);
        let tap_metrics_service = TapMetricsStateService::new(redis_repo.clone());
        let user_settings_cache = UserSettingsStateService::new(redis_repo.clone());

        let tap_service = TapService::new(
            tap_repo.clone(),
            user_repo.clone(),
            audit_log_service.clone(),
            tap_metrics_service.clone(),
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
        );

        Ok(Self {
            config: config.clone(),
            auth: AuthService::new(config.clone(), user_repo.clone()),
            tap: tap_service,
            api_key: api_key_service,
            notification: notification_service,
            audit_log: audit_log_service,
            tap_metrics: tap_metrics_service,
            verification: verification_service,
            user_settings: UserSettingsService::new(user_repo.clone(), user_settings_cache),
        })
    }
}
pub mod notification;
pub use notification::*;
