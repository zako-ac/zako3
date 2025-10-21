use crate::feature::{
    config::{ConfigRepository, MockConfigRepository},
    token::repository::{MockTokenRepository, TokenRepository},
    user::repository::{MockUserRepository, UserRepository},
};

pub trait ServiceRepository: Send + Sync {
    type UserRepository: UserRepository;
    type TokenRepository: TokenRepository;
    type ConfigRepository: ConfigRepository;
}

#[derive(Clone)]
pub struct Service<S: ServiceRepository> {
    pub config_repo: S::ConfigRepository,
    pub token_repo: S::TokenRepository,
    pub user_repo: S::UserRepository,
}

pub struct MockServiceRepository;

impl ServiceRepository for MockServiceRepository {
    type UserRepository = MockUserRepository;
    type TokenRepository = MockTokenRepository;
    type ConfigRepository = MockConfigRepository;
}

impl Service<MockServiceRepository> {
    pub fn modify<F>(self, f: F) -> Self
    where
        F: Fn(Self) -> Self,
    {
        f(self)
    }
}

impl MockServiceRepository {
    pub fn empty_service() -> Service<Self> {
        Service {
            config_repo: MockConfigRepository::new(),
            token_repo: MockTokenRepository::new(),
            user_repo: MockUserRepository::new(),
        }
    }

    pub fn modified_service<F>(f: F) -> Service<Self>
    where
        F: Fn(Service<Self>) -> Service<Self>,
    {
        f(Self::empty_service())
    }
}
