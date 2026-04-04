use thiserror::Error;
use zako3_types::{OnlineTapState, OnlineTapStates, hq::TapId};

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

    pub async fn get_tap_states(&self, tap_id: &TapId) -> Result<OnlineTapStates> {
        let key = format!("tap:{}", tap_id.0);

        let state_str = match self.cache_repository.get(&key).await {
            Some(state) => state,
            None => return Ok(vec![]),
        };

        let state: OnlineTapStates =
            serde_json::from_str(&state_str).map_err(|_| StateServiceError::CacheError)?;

        Ok(state)
    }

    pub async fn set_tap_states(&self, tap_id: &TapId, state: &OnlineTapStates) -> Result<()> {
        let key = format!("tap:{}", tap_id.0);

        let state_str = serde_json::to_string(state).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository.set(&key, &state_str).await;
        Ok(())
    }

    pub async fn set_connection_state(&self, state: OnlineTapState) -> Result<()> {
        let tap_id = state.tap_id.clone();

        let mut states = self.get_tap_states(&state.tap_id).await?;

        states.retain(|s| !(s.tap_id == state.tap_id && s.connection_id == state.connection_id));
        states.push(state);

        self.set_tap_states(&tap_id, &states).await?;

        Ok(())
    }

    pub async fn get_connection_state(
        &self,
        state: OnlineTapState,
    ) -> Result<Option<OnlineTapState>> {
        let states = self.get_tap_states(&state.tap_id).await?;
        let found = states
            .into_iter()
            .find(|s| s.tap_id == state.tap_id && s.connection_id == state.connection_id);

        Ok(found)
    }

    pub async fn remove_connection_state(&self, tap_id: &TapId, connection_id: u64) -> Result<()> {
        let mut states = self.get_tap_states(tap_id).await?;
        states.retain(|s| !(&s.tap_id == tap_id && s.connection_id == connection_id));
        self.set_tap_states(tap_id, &states).await?;

        Ok(())
    }
}
