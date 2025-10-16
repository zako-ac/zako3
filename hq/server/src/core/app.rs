use crate::{
    core::config::Config,
    feature::service::{Service, ServiceRepository},
    infrastructure::{postgres::PostgresDb, redis::RedisDb},
};

#[derive(Clone)]
pub struct AppTypes;

impl ServiceRepository for AppTypes {
    type UserRepository = PostgresDb;
    type TokenRepository = RedisDb;
    type ConfigRepository = Config;
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub service: Service<AppTypes>,
}
