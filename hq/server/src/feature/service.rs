use crate::{core::config::Config, feature::token::repository::TokenRepository};

pub trait ServiceRepository {
    type TokenRepository: TokenRepository;
}

pub struct Service<S: ServiceRepository> {
    pub config: Config,
    pub token_repo: S::TokenRepository,
}
