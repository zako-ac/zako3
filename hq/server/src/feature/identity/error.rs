use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

pub type IdentityResult<T> = Result<T, IdentityError>;
