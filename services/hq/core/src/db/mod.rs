use crate::CoreResult;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn get_pool(database_url: &str) -> CoreResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}
