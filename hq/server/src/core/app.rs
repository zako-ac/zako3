use crate::{
    core::config::Config,
    infrastructure::{postgres::PostgresDb, redis::RedisDb},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PostgresDb,
    pub redis: RedisDb,
}
