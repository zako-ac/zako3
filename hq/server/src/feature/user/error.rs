use thiserror::Error;

#[derive(Error, Debug)]
pub enum UserError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

pub type UserResult<T> = Result<T, UserError>;
