//! Collects preload sessions whose producer went away.
//!
//! Nothing did this before. Over HTTP it was survivable: a dying upload takes
//! its handler future with it, which drops the frame sender, so the writer at
//! least closes the file. But the session itself stayed in both maps forever —
//! and a stale `active_by_key` entry is not a mere leak, because
//! `GET /stream` prefers an "active" preload over the committed entry. Once one
//! is left behind, every later read of that cache key tails a dead partial file
//! until the process restarts.
//!
//! A UDP producer gives no close signal at all, so with datagram ingest this
//! stops being survivable and becomes the normal case.

use std::time::Duration;

use zako3_preload_cache::PreloadId;

use super::session;
use crate::server::state::AppState;

/// Start the background sweep.
pub fn spawn(state: AppState, ttl: Duration) {
    if ttl.is_zero() {
        tracing::warn!("preload session reaper disabled");
        return;
    }
    // Check several times per TTL so a collected session is not held far past
    // its deadline, without sweeping so often that it shows up in a profile.
    let interval = (ttl / 4).max(Duration::from_secs(1));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            sweep(&state, ttl).await;
        }
    });
}

/// Abort every session idle longer than `ttl`. Returns how many were collected.
pub async fn sweep(state: &AppState, ttl: Duration) -> usize {
    let stale: Vec<(PreloadId, String)> = state
        .sessions
        .iter()
        .filter(|e| e.value().idle_for() > ttl)
        .map(|e| (e.value().preload_id, e.value().item.tap_id.0.clone()))
        .collect();

    let mut collected = 0;
    for (id, tap_id) in stale {
        // `abort_session` clears both indexes and deletes the staged file,
        // which is exactly what a stalled producer needs.
        match session::abort_session(state, id).await {
            Ok(()) => {
                collected += 1;
                // Logged at info because a nonzero rate here means producers are
                // dying mid-upload, which is worth noticing rather than burying.
                tracing::info!(
                    preload_id = id.0,
                    %tap_id,
                    idle_secs = ttl.as_secs(),
                    "collected an abandoned preload session"
                );
            }
            Err(e) => tracing::warn!(%e, preload_id = id.0, "failed to collect preload session"),
        }
    }

    collected
}
