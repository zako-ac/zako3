//! In-process caching decorators for the mapper and pipeline repositories.
//!
//! Each HQ replica keeps a full in-memory copy of the registered WASM mappers and the
//! pipeline order, so the hot TTS path (`MapperRepository::find_by_id` per mapper +
//! `PipelineRepository::get_ordered`) never touches Postgres after warm-up.
//!
//! Consistency across replicas is maintained by:
//! - **write-through**: the replica handling a write updates its own cache and publishes a
//!   [`MapperCacheEvent`] after the DB commit;
//! - **pub/sub invalidation**: other replicas reload via [`CachedMapperRepository::refresh`]
//!   on receiving an event (wired in `service::mod`);
//! - **backstops**: refresh-on-(re)connect and a periodic safety refresh (wired in `service::mod`).
//!
//! Redis pub/sub is best-effort, so the publish step never fails a write — delivery gaps are
//! covered by the backstops.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tracing::warn;
use zako3_states::{MapperCacheEvent, RedisPubSub};
use zako3_tts_matching::{
    MapperRepository, PipelineRepository, Result as TtsResult, WasmMapper,
};

/// Publishes an invalidation event, logging (but never failing) on error.
async fn publish(pubsub: &Option<Arc<RedisPubSub>>, event: MapperCacheEvent) {
    if let Some(ps) = pubsub
        && let Err(e) = ps.publish_mapper_cache(&event).await
    {
        warn!(error = %e, "failed to publish mapper-cache invalidation");
    }
}

/// Caching decorator over a [`MapperRepository`].
pub struct CachedMapperRepository {
    inner: Arc<dyn MapperRepository>,
    pubsub: Option<Arc<RedisPubSub>>,
    cache: RwLock<HashMap<String, WasmMapper>>,
    loaded: AtomicBool,
    /// Serializes the cold-start bulk load so only one task issues the initial `list_all`.
    init_lock: tokio::sync::Mutex<()>,
}

impl CachedMapperRepository {
    pub fn new(inner: Arc<dyn MapperRepository>, pubsub: Option<Arc<RedisPubSub>>) -> Self {
        Self {
            inner,
            pubsub,
            cache: RwLock::new(HashMap::new()),
            loaded: AtomicBool::new(false),
            init_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Bulk-loads all mappers into the cache on first access (single query).
    async fn ensure_loaded(&self) -> TtsResult<()> {
        if self.loaded.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.init_lock.lock().await;
        // Re-check under the guard in case another task loaded while we waited.
        if self.loaded.load(Ordering::Acquire) {
            return Ok(());
        }
        let mappers = self.inner.list_all().await?;
        let map = mappers.into_iter().map(|m| (m.id.clone(), m)).collect();
        *self.cache.write().expect("mapper cache poisoned") = map;
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }

    /// Reloads the entire cache from the backing store (bulk query) and atomically swaps it in.
    ///
    /// Used by the pub/sub subscriber, refresh-on-connect, and the periodic safety task.
    pub async fn refresh(&self) -> TtsResult<()> {
        let mappers = self.inner.list_all().await?;
        let map = mappers.into_iter().map(|m| (m.id.clone(), m)).collect();
        *self.cache.write().expect("mapper cache poisoned") = map;
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait]
impl MapperRepository for CachedMapperRepository {
    async fn create(&self, mapper: WasmMapper) -> TtsResult<WasmMapper> {
        let created = self.inner.create(mapper).await?;
        self.cache
            .write()
            .expect("mapper cache poisoned")
            .insert(created.id.clone(), created.clone());
        publish(&self.pubsub, MapperCacheEvent::Changed(created.id.clone())).await;
        Ok(created)
    }

    async fn find_by_id(&self, id: &str) -> TtsResult<Option<WasmMapper>> {
        self.ensure_loaded().await?;
        Ok(self
            .cache
            .read()
            .expect("mapper cache poisoned")
            .get(id)
            .cloned())
    }

    async fn update(&self, mapper: WasmMapper) -> TtsResult<WasmMapper> {
        let updated = self.inner.update(mapper).await?;
        self.cache
            .write()
            .expect("mapper cache poisoned")
            .insert(updated.id.clone(), updated.clone());
        publish(&self.pubsub, MapperCacheEvent::Changed(updated.id.clone())).await;
        Ok(updated)
    }

    async fn delete(&self, id: &str) -> TtsResult<()> {
        self.inner.delete(id).await?;
        self.cache
            .write()
            .expect("mapper cache poisoned")
            .remove(id);
        publish(&self.pubsub, MapperCacheEvent::Deleted(id.to_string())).await;
        Ok(())
    }

    async fn list_all(&self) -> TtsResult<Vec<WasmMapper>> {
        self.ensure_loaded().await?;
        Ok(self
            .cache
            .read()
            .expect("mapper cache poisoned")
            .values()
            .cloned()
            .collect())
    }
}

/// Caching decorator over a [`PipelineRepository`].
pub struct CachedPipelineRepository {
    inner: Arc<dyn PipelineRepository>,
    pubsub: Option<Arc<RedisPubSub>>,
    cache: RwLock<Option<Vec<String>>>,
}

impl CachedPipelineRepository {
    pub fn new(inner: Arc<dyn PipelineRepository>, pubsub: Option<Arc<RedisPubSub>>) -> Self {
        Self {
            inner,
            pubsub,
            cache: RwLock::new(None),
        }
    }

    /// Reloads the pipeline order from the backing store and swaps it in.
    pub async fn refresh(&self) -> TtsResult<()> {
        let ids = self.inner.get_ordered().await?;
        *self.cache.write().expect("pipeline cache poisoned") = Some(ids);
        Ok(())
    }
}

#[async_trait]
impl PipelineRepository for CachedPipelineRepository {
    async fn get_ordered(&self) -> TtsResult<Vec<String>> {
        {
            let guard = self.cache.read().expect("pipeline cache poisoned");
            if let Some(ids) = guard.as_ref() {
                return Ok(ids.clone());
            }
        }
        let ids = self.inner.get_ordered().await?;
        *self.cache.write().expect("pipeline cache poisoned") = Some(ids.clone());
        Ok(ids)
    }

    async fn set_ordered(&self, mapper_ids: &[String]) -> TtsResult<()> {
        self.inner.set_ordered(mapper_ids).await?;
        *self.cache.write().expect("pipeline cache poisoned") = Some(mapper_ids.to_vec());
        publish(&self.pubsub, MapperCacheEvent::PipelineChanged).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    /// In-memory `MapperRepository` that counts how many times the backing store is queried.
    #[derive(Default)]
    struct FakeMapperRepo {
        mappers: Mutex<HashMap<String, WasmMapper>>,
        list_all_calls: AtomicUsize,
        find_calls: AtomicUsize,
    }

    fn mapper(id: &str, name: &str) -> WasmMapper {
        WasmMapper {
            id: id.to_string(),
            name: name.to_string(),
            wasm_bytes: vec![0, 1, 2, 3],
            sha256_hash: "deadbeef".to_string(),
            input_data: vec![],
        }
    }

    #[async_trait]
    impl MapperRepository for FakeMapperRepo {
        async fn create(&self, m: WasmMapper) -> TtsResult<WasmMapper> {
            self.mappers
                .lock()
                .unwrap()
                .insert(m.id.clone(), m.clone());
            Ok(m)
        }
        async fn find_by_id(&self, id: &str) -> TtsResult<Option<WasmMapper>> {
            self.find_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.mappers.lock().unwrap().get(id).cloned())
        }
        async fn update(&self, m: WasmMapper) -> TtsResult<WasmMapper> {
            self.mappers
                .lock()
                .unwrap()
                .insert(m.id.clone(), m.clone());
            Ok(m)
        }
        async fn delete(&self, id: &str) -> TtsResult<()> {
            self.mappers.lock().unwrap().remove(id);
            Ok(())
        }
        async fn list_all(&self) -> TtsResult<Vec<WasmMapper>> {
            self.list_all_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.mappers.lock().unwrap().values().cloned().collect())
        }
    }

    #[tokio::test]
    async fn cold_read_bulk_loads_once_then_serves_from_cache() {
        let inner = Arc::new(FakeMapperRepo::default());
        inner.mappers.lock().unwrap().insert("a".into(), mapper("a", "A"));
        inner.mappers.lock().unwrap().insert("b".into(), mapper("b", "B"));
        let cache = CachedMapperRepository::new(inner.clone(), None);

        // First read triggers exactly one bulk load.
        assert!(cache.find_by_id("a").await.unwrap().is_some());
        assert_eq!(inner.list_all_calls.load(Ordering::SeqCst), 1);

        // Subsequent reads hit memory only — no further bulk loads, never inner.find_by_id.
        assert!(cache.find_by_id("b").await.unwrap().is_some());
        assert_eq!(cache.list_all().await.unwrap().len(), 2);
        assert_eq!(inner.list_all_calls.load(Ordering::SeqCst), 1);
        assert_eq!(inner.find_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn writes_are_reflected_without_inner_reads() {
        let inner = Arc::new(FakeMapperRepo::default());
        let cache = CachedMapperRepository::new(inner.clone(), None);

        cache.create(mapper("a", "A")).await.unwrap();
        cache.update(mapper("a", "A2")).await.unwrap();
        assert_eq!(cache.find_by_id("a").await.unwrap().unwrap().name, "A2");

        cache.delete("a").await.unwrap();
        assert!(cache.find_by_id("a").await.unwrap().is_none());

        // No bulk load was needed: create() marked the cache populated implicitly via
        // write-through, but ensure_loaded() still runs once on the first read.
        assert!(inner.list_all_calls.load(Ordering::SeqCst) <= 1);
        assert_eq!(inner.find_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn refresh_picks_up_out_of_band_change() {
        let inner = Arc::new(FakeMapperRepo::default());
        inner.mappers.lock().unwrap().insert("a".into(), mapper("a", "A"));
        let cache = CachedMapperRepository::new(inner.clone(), None);

        assert_eq!(cache.find_by_id("a").await.unwrap().unwrap().name, "A");

        // Mutate the backing store directly (simulating another replica's write).
        inner.mappers.lock().unwrap().insert("a".into(), mapper("a", "A-new"));
        // Stale until refresh.
        assert_eq!(cache.find_by_id("a").await.unwrap().unwrap().name, "A");

        cache.refresh().await.unwrap();
        assert_eq!(cache.find_by_id("a").await.unwrap().unwrap().name, "A-new");
    }
}
