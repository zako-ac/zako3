use std::sync::Arc;

use crate::{
    feature::{
        auth::service::AuthService,
        config::model::AppConfig,
        token::repository::TokenRepository,
        user::{repository::UserRepository, service::UserService},
    },
    infrastructure::{postgres::PostgresDb, redis::RedisDb},
};

pub trait ServiceRepository {
    type UserRepository: UserRepository;
    type TokenRepository: TokenRepository;
}

pub struct AppTypes;

impl ServiceRepository for AppTypes {
    type UserRepository = PostgresDb;
    type TokenRepository = RedisDb;
}

pub struct Service<T>
where
    T: ServiceRepository,
{
    pub user_service: UserService<T::UserRepository>,
    pub auth_service: AuthService<T::TokenRepository>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub service: Arc<Service<AppTypes>>,
}

pub fn make_app_service(
    config: AppConfig,
    postgres: PostgresDb,
    redis: RedisDb,
) -> Service<AppTypes> {
    let user_service = UserService {
        user_repo: postgres.clone(),
    };

    let auth_service = AuthService {
        jwt_config: config.jwt.clone(),
        debug_config: config.debug.clone(),
        token_repo: redis.clone(),
    };

    Service {
        user_service,
        auth_service,
    }
}
