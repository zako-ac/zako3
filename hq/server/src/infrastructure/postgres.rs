use sqlx::PgPool;

pub mod settings;
pub mod user;

#[derive(Clone, Debug)]
pub struct PostgresDb {
    pub pool: PgPool,
}

pub async fn connect_postgres(connection_str: &str) -> Result<PostgresDb, sqlx::Error> {
    let pool = PgPool::connect(connection_str).await?;
    Ok(PostgresDb { pool })
}

pub async fn migrate_postgres(db: &PostgresDb) -> Result<(), sqlx::Error> {
    sqlx::migrate!().run(&db.pool).await?;

    Ok(())
}
