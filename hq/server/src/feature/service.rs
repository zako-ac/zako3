use crate::{core::config::Config, feature::token::repository::TokenRepository};

pub trait ServiceRepository: Send + Sync {
    type TokenRepository: TokenRepository;
}

pub struct Service<S: ServiceRepository> {
    pub config: Config,
    pub token_repo: S::TokenRepository,
}
