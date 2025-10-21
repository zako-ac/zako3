use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unknown auth error: {0}")]
    Unknown(String),

    #[error("expired access token")]
    ExpiredAccessToken,

    #[error("invalid refresh token")]
    InvalidRefreshToken,

    #[error("user not exists")]
    UserNotExists,

    #[error("insufficient privileges")]
    InsufficientPrivileges,

    #[error("attempt to login test account")]
    AttemptToLoginTestAccount,

    #[error("you are not allowed to use the service. your account is suspended")]
    NotAllowedService,
}
