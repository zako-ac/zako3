use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum UserError {
    #[error("duplicate name")]
    DuplicateName,
}
