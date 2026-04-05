pub mod auth;
pub mod tap;
pub mod tap_metric;

pub use auth::AuthService;
pub mod api_key;
pub use api_key::ApiKeyService;
pub mod audit_log;
pub use audit_log::AuditLogService;
pub use auth::Claims; // Export Claims
pub use tap::TapService;
pub use tap_metric::TapMetricService;
pub mod verification;
pub use verification::VerificationService;

use crate::repo::{
    PgApiKeyRepository, PgAuditLogRepo, PgTapRepository, PgUserRepository, TapMetricRepository,
};
use crate::{AppConfig, CoreResult};
use sqlx::PgPool;
use std::sync::Arc;

use zako3_states::TapMetricsStateService;

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
}

impl Service {
    pub async fn new(pool: PgPool, config: Arc<AppConfig>) -> CoreResult<Self> {
        let user_repo = Arc::new(PgUserRepository::new(pool.clone()));
        let tap_repo = Arc::new(PgTapRepository::new(pool.clone()));
        let api_key_repo = Arc::new(PgApiKeyRepository::new(pool.clone()));
        let audit_log_repo = Arc::new(PgAuditLogRepo::new(pool.clone()));
        let tap_metric_repo = Arc::new(TapMetricRepository::new(pool.clone()));
        let verification_repo = Arc::new(crate::repo::PgVerificationRepository::new(pool.clone()));

        let audit_log_service = AuditLogService::new(audit_log_repo.clone());
        let tap_metric_service = Arc::new(TapMetricService::new(tap_metric_repo.clone()));
        let notification_repo = Arc::new(crate::repo::PgNotificationRepository::new(pool.clone()));
        let notification_service = NotificationService::new(notification_repo);

        let redis_url = &config.redis_url;
        let redis_repo = zako3_states::RedisCacheRepository::new(redis_url).await?;
        let tap_metrics_service = TapMetricsStateService::new(Arc::new(redis_repo));

        let tap_service = TapService::new(
            tap_repo.clone(),
            user_repo.clone(),
            audit_log_service.clone(),
            tap_metric_service.clone(),
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
        })
    }
}
pub mod notification;
pub use notification::*;
