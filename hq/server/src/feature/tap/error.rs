use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum TapError {
    #[error("duplicate name")]
    DuplicateName,
}
