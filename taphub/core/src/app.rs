use crate::repository::HqRepositoryRef;
use zako3_states::CacheRepositoryRef;

#[derive(Clone)]
pub struct App {
    pub hq_repository: HqRepositoryRef,
    pub cache_repository: CacheRepositoryRef,
}
