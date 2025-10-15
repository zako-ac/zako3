use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unknown auth error: {0}")]
    Unknown(String),

    #[error("expred access token")]
    ExpiredAccessToken,

    #[error("user not exists")]
    UserNotExists,

    #[error("insufficient previleges")]
    InsufficientPrevileges,

    #[error("you are not allowed to use the service")]
    NotAllowedService,
}
