use crate::repo::TapRepository;
use crate::CoreResult;
use hq_types::hq::{CreateTapDto, Tap};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct TapService {
    tap_repo: Arc<dyn TapRepository>,
}

impl TapService {
    pub fn new(tap_repo: Arc<dyn TapRepository>) -> Self {
        Self { tap_repo }
    }

    pub async fn create(&self, owner_id: Uuid, dto: CreateTapDto) -> CoreResult<Tap> {
        let tap = Tap::new(Uuid::new_v4(), owner_id, dto.name);
        self.tap_repo.create(&tap).await
    }

    pub async fn list_by_user(&self, user_id: Uuid) -> CoreResult<Vec<Tap>> {
        self.tap_repo.list_by_owner(user_id).await
    }
}
