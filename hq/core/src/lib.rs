pub mod config;
pub mod db;
pub mod error;
pub mod repo;
pub mod service;

pub use config::AppConfig;
pub use db::get_pool;
pub use error::{CoreError, CoreResult};
pub use service::{Claims, Service};
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run_migrations(pool: &sqlx::PgPool) -> CoreResult<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}
