use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::{StreamExt, stream};
use tokio::{fs, io, sync::RwLock};
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

/// How many sidecars are merged into the live index per write-lock acquisition.
const MERGE_BATCH: usize = 512;

/// Emit a progress log every this many scanned sidecars.
const PROGRESS_EVERY: usize = 5_000;

/// Sidecar reads in flight when no explicit concurrency is given.
pub const DEFAULT_WARMUP_CONCURRENCY: usize = 64;

/// Outcome of a [`CacheDb::warm_from_dir`] pass.
#[derive(Debug, Default, Clone)]
pub struct WarmupStats {
    /// `*.json` files found in the directory.
    pub total: usize,
    /// Distinct keys the scan added to the index.
    pub indexed: usize,
    /// Sidecars that lost to another sidecar (or a live write) for the same key.
    pub duplicates: usize,
    /// Sidecars left `is_downloading` by a killed write.
    pub stale_downloads: usize,
    /// Sidecars that failed to parse.
    pub parse_failures: usize,
    /// Unreferenced `.json`/`.opus` files deleted after the scan.
    pub reclaimed: usize,
    pub elapsed: Duration,
}

// ---------------------------------------------------------------------------
// MetaSidecar — JSON file written next to each .opus file.
// This is the single source of truth for all cache entry state.
// New fields use #[serde(default)] for backward-compat with old sidecars.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct MetaSidecar {
    pub tap_id: String,
    /// serde_json of `AudioCacheItemKey`
    pub cache_key: String,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_policy: AudioCachePolicy,
    /// Unix seconds UTC; `None` means no expiry.
    pub expire_at: Option<i64>,
    /// Unix seconds UTC.
    pub created_at: i64,
    #[serde(default)]
    pub use_count: i64,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub gdsf_priority: f64,
    /// True while the .opus file is still being written.
    #[serde(default)]
    pub is_downloading: bool,
    /// True when a companion .opus file exists alongside this .json.
    #[serde(default)]
    pub has_opus: bool,
}

// ---------------------------------------------------------------------------
// DbEntry — public query result type (unchanged API for cache-gc consumers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DbEntry {
    pub tap_id: String,
    /// serde_json of `AudioCacheItemKey`
    pub cache_key: String,
    /// Absolute path to the `.opus` file; `None` for metadata-only entries.
    pub opus_path: Option<String>,
    /// Absolute path to the `.json` sidecar file (always present).
    pub json_path: String,
    /// Unix seconds UTC; `None` means no expiry.
    pub expire_at: Option<i64>,
    pub use_count: i64,
    /// Unix seconds UTC.
    pub last_used_at: Option<i64>,
    /// Unix seconds UTC.
    pub created_at: i64,
    pub gdsf_priority: f64,
    pub is_downloading: bool,
}

// ---------------------------------------------------------------------------
// CacheDb — in-memory index backed by JSON sidecar files (no SQLite)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CacheDb {
    // key: (tap_id, cache_key_json)
    entries: Arc<RwLock<HashMap<(String, String), (PathBuf, MetaSidecar)>>>,
}

impl CacheDb {
    /// An index with no entries. Pair with [`CacheDb::warm_from_dir`] to populate it
    /// in the background while the server is already serving requests.
    pub fn empty() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build the index by scanning `dir` for `*.json` sidecar files.
    /// Silently removes any legacy `cache.db` SQLite file if found.
    pub async fn open(dir: &Path) -> io::Result<Self> {
        let db = Self::empty();
        db.warm_from_dir(dir, DEFAULT_WARMUP_CONCURRENCY).await?;
        Ok(db)
    }

    /// Populate the index from the `*.json` sidecars in `dir`, reading up to
    /// `concurrency` files at a time and merging them in batches.
    ///
    /// Safe to run while requests are being served: entries written since the
    /// process started always win over what is found on disk (see the merge rule
    /// in [`CacheDb::merge_batch`]), so a live write is never clobbered.
    pub async fn warm_from_dir(&self, dir: &Path, concurrency: usize) -> io::Result<WarmupStats> {
        self.warm_from_dir_with_progress(dir, concurrency, |_, _| {}).await
    }

    /// [`CacheDb::warm_from_dir`], reporting `(scanned, total)` after every batch.
    pub async fn warm_from_dir_with_progress(
        &self,
        dir: &Path,
        concurrency: usize,
        progress: impl Fn(usize, usize),
    ) -> io::Result<WarmupStats> {
        let started = std::time::Instant::now();
        remove_legacy_sqlite_db(dir).await;

        let paths = collect_json_paths(dir).await?;
        let total = paths.len();
        let concurrency = concurrency.max(1);
        progress(0, total);
        tracing::info!(
            dir = %dir.display(),
            total,
            concurrency,
            "cache index warmup started"
        );

        let mut stats = WarmupStats {
            total,
            ..Default::default()
        };
        // Deleted only after the scan ends, so a concurrent request never races it.
        let mut reclaimable: Vec<PathBuf> = Vec::new();
        let mut scan_keys: HashSet<(String, String)> = HashSet::new();

        let mut loaded = stream::iter(paths)
            .map(|path| async move {
                let res = load_sidecar(&path).await;
                (path, res)
            })
            .buffer_unordered(concurrency);

        let mut batch: Vec<(PathBuf, MetaSidecar)> = Vec::with_capacity(MERGE_BATCH);
        let mut scanned = 0usize;
        let mut last_logged = 0usize;

        while let Some((path, res)) = loaded.next().await {
            scanned += 1;
            match res {
                // Leftover from a killed write. Every GC pass skips `is_downloading`,
                // so indexing it would leak it forever.
                Ok(sidecar) if sidecar.is_downloading => {
                    stats.stale_downloads += 1;
                    reclaimable.push(path);
                }
                Ok(sidecar) => batch.push((path, sidecar)),
                Err(e) => {
                    stats.parse_failures += 1;
                    tracing::warn!(path = %path.display(), %e, "failed to parse sidecar, skipping");
                }
            }

            if batch.len() >= MERGE_BATCH {
                self.merge_batch(
                    std::mem::take(&mut batch),
                    &mut stats,
                    &mut reclaimable,
                    &mut scan_keys,
                )
                .await;
                progress(scanned, total);
            }

            if scanned - last_logged >= PROGRESS_EVERY {
                last_logged = scanned;
                tracing::info!(
                    scanned,
                    total,
                    indexed = stats.indexed,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "cache index warmup progress"
                );
            }
        }

        self.merge_batch(batch, &mut stats, &mut reclaimable, &mut scan_keys).await;
        progress(scanned, total);

        stats.reclaimed = reclaim_files(&reclaimable).await;
        stats.elapsed = started.elapsed();

        tracing::info!(
            total = stats.total,
            indexed = stats.indexed,
            duplicates = stats.duplicates,
            stale_downloads = stats.stale_downloads,
            parse_failures = stats.parse_failures,
            reclaimed = stats.reclaimed,
            elapsed_ms = stats.elapsed.as_millis() as u64,
            "cache index warmup complete"
        );
        Ok(stats)
    }

    /// Merge scanned sidecars into the live index.
    ///
    /// Merge rule: a scanned sidecar is inserted only when its key is absent, or
    /// when its `created_at` is *strictly* newer than the incumbent's — ties keep
    /// the incumbent. Anything written since the process started carries a
    /// `created_at` at or after boot, which is >= every sidecar that was already on
    /// disk, so this one rule both protects live writes and resolves duplicate
    /// sidecars for the same key in favour of the newest.
    async fn merge_batch(
        &self,
        batch: Vec<(PathBuf, MetaSidecar)>,
        stats: &mut WarmupStats,
        reclaimable: &mut Vec<PathBuf>,
        scan_keys: &mut HashSet<(String, String)>,
    ) {
        if batch.is_empty() {
            return;
        }
        let mut map = self.entries.write().await;
        for (path, sidecar) in batch {
            let key = (sidecar.tap_id.clone(), sidecar.cache_key.clone());
            match map.get(&key) {
                Some((incumbent_path, incumbent)) if incumbent.created_at >= sidecar.created_at => {
                    // Same file, already indexed by a live request. Reclaiming it
                    // would delete a live entry's own data.
                    if *incumbent_path == path {
                        continue;
                    }
                    stats.duplicates += 1;
                    if !incumbent.is_downloading {
                        reclaimable.push(path);
                    }
                }
                Some(_) => {
                    stats.duplicates += 1;
                    // Only reclaim a loser this scan put there; one from a live
                    // request may still be referenced.
                    let scan_owned = scan_keys.contains(&key);
                    if let Some((old_path, _)) = map.insert(key.clone(), (path, sidecar))
                        && scan_owned
                    {
                        reclaimable.push(old_path);
                    }
                    scan_keys.insert(key);
                }
                None => {
                    map.insert(key.clone(), (path, sidecar));
                    scan_keys.insert(key);
                    stats.indexed += 1;
                }
            }
        }
    }

    /// Insert a `DbEntry` into the index, creating a minimal JSON sidecar on disk.
    /// Used by external tools and tests that construct entries without full metadata.
    pub async fn insert(&self, entry: DbEntry) -> io::Result<()> {
        let sidecar = MetaSidecar {
            tap_id: entry.tap_id.clone(),
            cache_key: entry.cache_key.clone(),
            metadatas: vec![],
            cache_policy: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            expire_at: entry.expire_at,
            created_at: entry.created_at,
            use_count: entry.use_count,
            last_used_at: entry.last_used_at,
            gdsf_priority: entry.gdsf_priority,
            is_downloading: entry.is_downloading,
            has_opus: entry.opus_path.is_some(),
        };
        let json_path = PathBuf::from(&entry.json_path);
        self.insert_sidecar(json_path, sidecar).await
    }

    /// Write `sidecar` to `json_path` and register it in the index.
    /// Used internally by `FileAudioCache` to persist full metadata.
    pub(crate) async fn insert_sidecar(&self, json_path: PathBuf, sidecar: MetaSidecar) -> io::Result<()> {
        write_sidecar(&json_path, &sidecar).await?;
        let key = (sidecar.tap_id.clone(), sidecar.cache_key.clone());
        self.entries.write().await.insert(key, (json_path, sidecar));
        Ok(())
    }

    /// Mark an entry as fully written (`is_downloading = false`, `has_opus = true`).
    pub async fn mark_complete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.is_downloading = false;
            e.1.has_opus = true;
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Look up an entry by `(tap_id, cache_key)`.
    pub async fn get(&self, tap_id: String, cache_key: String) -> io::Result<Option<DbEntry>> {
        let map = self.entries.read().await;
        Ok(map.get(&(tap_id, cache_key)).map(|(p, s)| to_db_entry(p, s)))
    }

    /// Look up the full sidecar (includes metadatas and cache_policy).
    pub(crate) async fn get_sidecar(
        &self,
        tap_id: String,
        cache_key: String,
    ) -> Option<MetaSidecar> {
        let map = self.entries.read().await;
        map.get(&(tap_id, cache_key)).map(|(_, s)| s.clone())
    }

    /// Remove an entry from the index. Does **not** delete files (caller's responsibility).
    pub async fn delete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        self.entries.write().await.remove(&(tap_id, cache_key));
        Ok(())
    }

    /// Remove every entry belonging to `tap_id` from the index and return the
    /// removed entries so the caller can delete their `.opus`/`.json` files.
    /// Does **not** delete files (caller's responsibility).
    pub async fn delete_all_for_tap(&self, tap_id: &str) -> io::Result<Vec<DbEntry>> {
        let mut map = self.entries.write().await;
        let keys: Vec<(String, String)> = map
            .keys()
            .filter(|(tid, _)| tid == tap_id)
            .cloned()
            .collect();
        let mut removed = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some((path, sidecar)) = map.remove(&key) {
                removed.push(to_db_entry(&path, &sidecar));
            }
        }
        Ok(removed)
    }

    /// Increment `use_count` and update `last_used_at`.
    pub async fn touch(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let now = chrono::Utc::now().timestamp();
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.use_count += 1;
            e.1.last_used_at = Some(now);
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Set the GDSF eviction priority for an entry.
    pub async fn set_gdsf_priority(
        &self,
        tap_id: String,
        cache_key: String,
        priority: f64,
    ) -> io::Result<()> {
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.gdsf_priority = priority;
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Return every entry in the index.
    pub async fn get_all_entries(&self) -> io::Result<Vec<DbEntry>> {
        let map = self.entries.read().await;
        Ok(map.values().map(|(p, s)| to_db_entry(p, s)).collect())
    }

    /// Return all opus paths for complete (non-downloading, has_opus) entries.
    pub async fn get_all_opus_paths(&self) -> io::Result<Vec<String>> {
        let map = self.entries.read().await;
        Ok(map
            .values()
            .filter(|(_, s)| s.has_opus && !s.is_downloading)
            .map(|(p, _)| p.with_extension("opus").to_string_lossy().into_owned())
            .collect())
    }

    /// Return all json_path values.
    pub async fn get_all_json_paths(&self) -> io::Result<Vec<String>> {
        let map = self.entries.read().await;
        Ok(map
            .values()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect())
    }

    /// Return up to `limit` complete entries with the lowest GDSF priority (eviction candidates).
    pub async fn get_lowest_priority_entries(&self, limit: usize) -> io::Result<Vec<DbEntry>> {
        let map = self.entries.read().await;
        let mut candidates: Vec<DbEntry> = map
            .values()
            .filter(|(_, s)| s.has_opus && !s.is_downloading)
            .map(|(p, s)| to_db_entry(p, s))
            .collect();
        candidates.sort_by(|a, b| {
            a.gdsf_priority
                .partial_cmp(&b.gdsf_priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        Ok(candidates)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_db_entry(json_path: &Path, s: &MetaSidecar) -> DbEntry {
    DbEntry {
        tap_id: s.tap_id.clone(),
        cache_key: s.cache_key.clone(),
        opus_path: if s.has_opus {
            Some(json_path.with_extension("opus").to_string_lossy().into_owned())
        } else {
            None
        },
        json_path: json_path.to_string_lossy().into_owned(),
        expire_at: s.expire_at,
        use_count: s.use_count,
        last_used_at: s.last_used_at,
        created_at: s.created_at,
        gdsf_priority: s.gdsf_priority,
        is_downloading: s.is_downloading,
    }
}

/// Migration: drop the `cache.db` left behind by the SQLite-backed index.
async fn remove_legacy_sqlite_db(dir: &Path) {
    let db_path = dir.join("cache.db");
    match fs::try_exists(&db_path).await {
        Ok(true) => {
            if let Err(e) = fs::remove_file(&db_path).await {
                tracing::warn!(path = %db_path.display(), %e, "failed to remove legacy cache.db");
            } else {
                tracing::info!(path = %db_path.display(), "removed legacy SQLite cache.db");
            }
        }
        Ok(false) => {}
        Err(e) => tracing::warn!(path = %db_path.display(), %e, "failed to stat legacy cache.db"),
    }
}

/// One pass over the directory stream collecting `*.json` paths. No per-file I/O
/// happens here — the reads are what gets parallelised afterwards.
async fn collect_json_paths(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut dir_read = match fs::read_dir(dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut paths = Vec::new();
    while let Some(entry) = dir_read.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Delete unreferenced sidecars and their `.opus` siblings. Returns how many
/// files were removed.
async fn reclaim_files(paths: &[PathBuf]) -> usize {
    let mut removed = 0;
    for json_path in paths {
        for path in [json_path.clone(), json_path.with_extension("opus")] {
            match fs::remove_file(&path).await {
                Ok(()) => removed += 1,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(path = %path.display(), %e, "failed to reclaim orphaned cache file");
                }
            }
        }
    }
    if removed > 0 {
        tracing::info!(removed, "reclaimed orphaned cache files after warmup");
    }
    removed
}

async fn load_sidecar(path: &Path) -> io::Result<MetaSidecar> {
    let bytes = fs::read(path).await?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub(crate) async fn write_sidecar(path: &Path, sidecar: &MetaSidecar) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let json =
        serde_json::to_vec(sidecar).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut file = fs::File::create(path).await?;
    file.write_all(&json).await?;
    file.flush().await?;
    file.sync_data().await?;
    Ok(())
}
