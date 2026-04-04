use crate::repository::{CacheRepositoryRef, HqRepositoryRef};

#[derive(Clone)]
pub struct App {
    pub hq_repository: HqRepositoryRef,
    pub cache_repository: CacheRepositoryRef,
}
