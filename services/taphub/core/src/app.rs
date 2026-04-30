use crate::repository::HqRepositoryRef;
use zako3_metrics::TapRedisMetrics;
use zako3_states::{CacheRepositoryRef, TapHubStateService};

#[derive(Clone)]
pub struct App {
    pub hq_repository: HqRepositoryRef,
    pub cache_repository: CacheRepositoryRef,
    pub tap_state_service: TapHubStateService,
    pub tap_metrics_service: TapRedisMetrics,
}
