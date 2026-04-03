use crate::repository::{CacheRepositoryRef, HqRepositoryRef};

pub struct App {
    pub hq_repository: HqRepositoryRef,
    pub cache_repository: CacheRepositoryRef,
}
