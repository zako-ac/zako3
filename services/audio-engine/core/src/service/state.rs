use crate::{
    error::ZakoResult,
    types::{ChannelId, GuildId, SessionState},
};
use async_trait::async_trait;
use dashmap::{DashMap, mapref::entry::Entry};
use mockall::automock;
use std::{
    sync::{Arc, LazyLock, Weak},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;

pub type ArcStateService = Arc<dyn StateService>;

#[automock]
#[async_trait]
pub trait StateService: Send + Sync + 'static {
    async fn get_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>>;
    async fn save_session(&self, session: &SessionState) -> ZakoResult<()>;
    async fn delete_session(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()>;
    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>>;
    async fn list_sessions_in_guild(&self, guild_id: GuildId) -> ZakoResult<Vec<SessionState>>;
}

/// A writer that waits longer than this for the session write lock gets a
/// warning. The critical section is a pure in-memory get → mutate → save, so
/// waiting anywhere near this long means somebody is holding the lock across a
/// slow await — such a wait shows up as latency, never as an error.
const SLOW_SESSION_LOCK_WAIT: Duration = Duration::from_millis(25);

/// Per-session write locks, keyed by `(guild_id, channel_id)`.
///
/// ## What the lock has to cover
/// A session is stored twice: the in-memory mirror (`DashMap` in
/// `InMemoryStateService` / `RedisStateService`) is the authoritative runtime
/// view, and Redis is a best-effort copy written **from** it inside
/// `save_session` (`redis_state.rs`). Both layers are written by one
/// `save_session` call, so holding this lock across get → mutate → save
/// serialises the read and both writes: the DashMap entry and the Redis `SET`.
///
/// A lock inside a single backend would not be enough: `modify_state_session`
/// only sees an `Arc<dyn StateService>`, and two writers can arrive through
/// different handles. The lock is therefore process-wide and keyed by the
/// session, not by the store object.
///
/// Cross-process: a session belongs to the AE process that owns the voice
/// connection for that channel, and Redis is never read-modify-written here
/// (it is a mirror, not the source of truth), so a process-local lock is the
/// right scope and there is no cross-process lost-update path through it.
///
/// ## Lock order (never invert)
/// 1. `SessionControl::reconcile_guard` — held across a whole `reconcile()`.
/// 2. the session write lock — held **only** for get → mutate → save.
///
/// `reconcile()` mutates the session for a failed track while holding
/// `reconcile_guard`, so it takes 1 then 2. No task holding the session write
/// lock calls `reconcile()`: the write helpers return (and release the lock)
/// before their caller reconciles, so 2 is never followed by 1 and the two
/// cannot deadlock. The `concurrent_play_and_stop_do_not_deadlock` test in
/// this module is the guard for that claim.
static SESSION_LOCKS: LazyLock<DashMap<(GuildId, ChannelId), Weak<AsyncMutex<()>>>> =
    LazyLock::new(DashMap::new);

/// The write lock for one session, created on first use.
///
/// The registry holds a `Weak`, so a channel that is never used again does not
/// keep a lock alive forever; an entry whose lock is still held (or still
/// handed out) stays, so two concurrent first-writers always get the *same*
/// lock.
fn session_lock(guild_id: GuildId, channel_id: ChannelId) -> Arc<AsyncMutex<()>> {
    // One entry lock: a get-then-insert would let two writers race and each
    // create its own mutex, which is exactly the lost update this lock exists
    // to prevent.
    match SESSION_LOCKS.entry((guild_id, channel_id)) {
        Entry::Occupied(mut occupied) => {
            if let Some(lock) = occupied.get().upgrade() {
                return lock;
            }
            let lock = Arc::new(AsyncMutex::new(()));
            occupied.insert(Arc::downgrade(&lock));
            lock
        }
        Entry::Vacant(vacant) => {
            let lock = Arc::new(AsyncMutex::new(()));
            vacant.insert(Arc::downgrade(&lock));
            lock
        }
    }
}

/// Read-modify-write one session atomically and return whatever `f` produced.
///
/// Returns `Ok(None)` when the session does not exist; the closure is not
/// called in that case (creating a session stays a separate, explicit act).
pub async fn with_state_session<F, R>(
    state_service: &ArcStateService,
    guild_id: GuildId,
    channel_id: ChannelId,
    f: F,
) -> ZakoResult<Option<R>>
where
    F: FnOnce(&mut SessionState) -> R + Send,
    R: Send,
{
    let lock = session_lock(guild_id, channel_id);

    let wait_started = Instant::now();
    let _guard = lock.lock().await;
    let waited = wait_started.elapsed();
    if waited >= SLOW_SESSION_LOCK_WAIT {
        tracing::warn!(
            guild_id = %guild_id,
            channel_id = %channel_id,
            waited_ms = waited.as_millis() as u64,
            "Waited for the session state write lock; a writer is holding it too long"
        );
    } else {
        tracing::trace!(
            guild_id = %guild_id,
            channel_id = %channel_id,
            waited_us = waited.as_micros() as u64,
            "Acquired the session state write lock"
        );
    }

    let Some(mut session) = state_service.get_session(guild_id, channel_id).await? else {
        return Ok(None);
    };

    let out = f(&mut session);

    // Covers both layers: `save_session` writes the in-memory mirror and then
    // mirrors the same value to Redis.
    let save_started = Instant::now();
    state_service.save_session(&session).await?;
    tracing::trace!(
        guild_id = %guild_id,
        channel_id = %channel_id,
        save_us = save_started.elapsed().as_micros() as u64,
        "Released the session state write lock"
    );

    Ok(Some(out))
}

/// Read-modify-write one session atomically.
///
/// Every mutation of `SessionState` has to go through this or
/// [`with_state_session`]: a bare `get_session` … `save_session` pair drops a
/// concurrent writer's change silently, which is what made simultaneous plays
/// lose queue entries with no log line anywhere.
pub async fn modify_state_session<F>(
    state_service: &ArcStateService,
    guild_id: GuildId,
    channel_id: ChannelId,
    f: F,
) -> ZakoResult<()>
where
    F: FnOnce(&mut SessionState) + Send,
{
    with_state_session(state_service, guild_id, channel_id, f)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests;
