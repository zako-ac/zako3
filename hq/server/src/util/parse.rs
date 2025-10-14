use crate::util::error::{AppError, AppResult};

pub fn parse_u64(string: &str) -> AppResult<u64> {
    string
        .parse::<u64>()
        .map_err(|_| AppError::Unknown(format!("expected u64, got: {}", string)))
}
