pub mod auth;
pub mod tap;

pub use auth::AuthService;
pub use auth::Claims; // Export Claims
pub use tap::TapService;

use crate::repo::{PgTapRepository, PgUserRepository};
use crate::{AppConfig, CoreResult};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Service {
    pub config: Arc<AppConfig>,
    pub auth: AuthService,
    pub tap: TapService,
}

impl Service {
    pub async fn new(pool: PgPool, config: Arc<AppConfig>) -> CoreResult<Self> {
        let user_repo = Arc::new(PgUserRepository::new(pool.clone()));
        let tap_repo = Arc::new(PgTapRepository::new(pool.clone()));

        Ok(Self {
            config: config.clone(),
            auth: AuthService::new(config.clone(), user_repo.clone()),
            tap: TapService::new(tap_repo, user_repo),
        })
    }
}
