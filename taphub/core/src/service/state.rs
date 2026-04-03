use thiserror::Error;
use zako3_types::{OnlineTapStates, hq::TapId};

use crate::repository::CacheRepositoryRef;

#[derive(Error, Debug)]
pub enum StateServiceError {
    #[error("Cache error")]
    CacheError,
}

type Result<T> = std::result::Result<T, StateServiceError>;

#[derive(Clone)]
pub struct StateService {
    pub cache_repository: CacheRepositoryRef,
}

impl StateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    pub async fn get_tap_state(&self, tap_id: &TapId) -> Result<OnlineTapStates> {
        let state_str = match self.cache_repository.get(&tap_id.0.to_string()).await {
            Some(state) => state,
            None => return Ok(vec![]),
        };

        let state: OnlineTapStates =
            serde_json::from_str(&state_str).map_err(|_| StateServiceError::CacheError)?;

        Ok(state)
    }

    pub async fn set_tap_state(&self, tap_id: &TapId, state: &OnlineTapStates) -> Result<()> {
        let state_str = serde_json::to_string(state).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository
            .set(&tap_id.0.to_string(), &state_str)
            .await;
        Ok(())
    }
}
