//! HQ's registry of audio engines: who exists, where to reach them, and which
//! one should serve a given voice session.
//!
//! Engines announce themselves (see `ae_protocol::AeRegistryRpc`); HQ keeps a
//! live set and never learns about an engine any other way. An engine that
//! stops heartbeating simply stops being a placement candidate, so a crashed
//! pod drops out without any explicit deregistration or cross-service
//! reconciliation.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ae_protocol::{
    AeAdvertisement, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineRpcClient, SessionInfo,
};
use hq_types::GuildId;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use rustc_hash::FxHasher;
use thiserror::Error;
use tokio::sync::RwLock;

/// How long an engine stays a placement candidate after its last heartbeat.
///
/// Engines heartbeat every 15 s, so this tolerates two missed beats before HQ
/// stops sending work to them — long enough to ride out a brief RPC hiccup,
/// short enough that a dead pod leaves the pool in well under a minute.
pub const HEARTBEAT_TTL: Duration = Duration::from_secs(45);

/// HQ's budget for one command dispatch to one engine.
///
/// This is the *outer* layer of the timeout stack. An engine's own command
/// backstop is 25 s (`services/audio-engine/controller/src/server.rs`), which
/// in turn sits above the worst-case taphub budget of a single command. HQ's
/// timeout must stay above the engine's backstop so a slow-but-alive engine
/// always answers with a structured error instead of being cut off by HQ and
/// surfacing as a transport failure.
///
/// Changing any layer means re-checking the whole chain:
///   2 × MAX_ATTEMPTS(2) × taphub_per_attempt(6s) = 24s ≤ engine backstop(25s)
///     < HQ dispatch(30s)
pub const AE_DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);

/// How long an engine is given to answer the registration call itself.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum AeError {
    #[error("Already in VC")]
    AlreadyJoined,
    #[error("Not in VC")]
    NotJoined,
    #[error("Permission denied")]
    PermissionDenied,
    #[error(transparent)]
    Tap(#[from] hq_types::TapHubError),
    #[error("no audio engine is registered")]
    NoEngine,
    #[error("audio engine transport error: {0}")]
    Transport(String),
}

/// One registered engine, as HQ sees it.
#[derive(Clone, Debug)]
pub struct AeEntry {
    /// Stable engine identity — the StatefulSet pod name in a cluster.
    pub sink_id: String,
    /// Discord user id of the bot this engine is logged in as.
    pub client_id: String,
    pub addr: String,
    /// Guilds this engine's bot is currently a member of, as self-reported.
    pub allowed_guilds: HashSet<GuildId>,
    pub last_heartbeat: Instant,
    pub client: HttpClient,
}

impl AeEntry {
    /// Whether HQ has heard from this engine recently enough to keep using it.
    pub fn is_fresh(&self) -> bool {
        self.last_heartbeat.elapsed() < HEARTBEAT_TTL
    }

    pub fn is_permitted_for(&self, guild_id: &GuildId) -> bool {
        self.allowed_guilds.contains(guild_id)
    }
}

fn build_client(addr: &str) -> Result<HttpClient, AeError> {
    let url = if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("http://{addr}")
    };
    HttpClientBuilder::default()
        .request_timeout(AE_DISPATCH_TIMEOUT)
        .build(&url)
        .map_err(|e| AeError::Transport(format!("failed to build client for {url}: {e}")))
}

#[derive(Default)]
pub struct AeRegistry {
    entries: RwLock<HashMap<String, AeEntry>>,
}

impl AeRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// First contact from an engine after it has logged into Discord.
    ///
    /// Idempotent: a restart re-registers over the previous entry for the same
    /// `sink_id`. The guild permission set is kept when the same bot comes
    /// back (a pod restart is not a membership change) and cleared when a
    /// different bot shows up under the same engine id, since the old bot's
    /// guilds say nothing about the new one.
    pub async fn register(&self, advertisement: AeAdvertisement) -> Result<(), AeError> {
        self.upsert(advertisement, true).await
    }

    /// Keeps an entry eligible. Never invalidates anything else about it.
    pub async fn heartbeat(&self, advertisement: AeAdvertisement) -> Result<(), AeError> {
        self.upsert(advertisement, false).await
    }

    async fn upsert(
        &self,
        advertisement: AeAdvertisement,
        is_registration: bool,
    ) -> Result<(), AeError> {
        let mut entries = self.entries.write().await;

        // Take the existing entry out rather than holding a mutable borrow of
        // the map across the rebuild: the address may have moved, and the new
        // client has to be built before it replaces the old one.
        let existing = entries.remove(&advertisement.sink_id);

        let mut entry = match existing {
            Some(entry) => entry,
            None => {
                tracing::info!(
                    sink_id = %advertisement.sink_id,
                    client_id = %advertisement.client_id,
                    addr = %advertisement.advertise_addr,
                    "audio engine registered"
                );
                AeEntry {
                    sink_id: advertisement.sink_id.clone(),
                    client_id: advertisement.client_id.clone(),
                    addr: advertisement.advertise_addr.clone(),
                    allowed_guilds: HashSet::new(),
                    last_heartbeat: Instant::now(),
                    client: build_client(&advertisement.advertise_addr)?,
                }
            }
        };

        // Rebuild the client only when the address actually moved: a heartbeat
        // must not tear down a live connection pool, or a concurrent dispatch
        // can land in the gap and look like a dead engine.
        if entry.addr != advertisement.advertise_addr {
            entry.client = build_client(&advertisement.advertise_addr)?;
            entry.addr = advertisement.advertise_addr.clone();
        }

        // A registration from a *different* bot under the same engine id is a
        // different bot's membership list; a restart of the same bot is not.
        if is_registration && entry.client_id != advertisement.client_id {
            entry.allowed_guilds.clear();
        }

        entry.client_id = advertisement.client_id.clone();
        entry.last_heartbeat = Instant::now();
        tracing::debug!(
            sink_id = %entry.sink_id,
            client_id = %entry.client_id,
            addr = %entry.addr,
            registration = is_registration,
            "audio engine registration refreshed"
        );

        entries.insert(advertisement.sink_id, entry);
        Ok(())
    }

    /// The guilds an engine's bot is currently a member of.
    pub async fn report_guilds(&self, client_id: &str, guilds: Vec<GuildId>) {
        let mut entries = self.entries.write().await;
        match entries.values_mut().find(|e| e.client_id == client_id) {
            Some(entry) => {
                tracing::debug!(
                    sink_id = %entry.sink_id,
                    client_id = %client_id,
                    guild_count = guilds.len(),
                    "audio engine guild permissions updated"
                );
                entry.allowed_guilds = guilds.into_iter().collect();
            }
            None => {
                tracing::warn!(
                    client_id = %client_id,
                    "report_guilds from an engine HQ has never seen register"
                );
            }
        }
    }

    /// Every engine HQ has heard from recently, in no particular order.
    pub async fn entries(&self) -> Vec<AeEntry> {
        self.entries
            .read()
            .await
            .values()
            .filter(|e| e.is_fresh())
            .cloned()
            .collect()
    }

    /// The engine currently logged in as `client_id`.
    pub async fn entry_by_client_id(&self, client_id: &str) -> Option<AeEntry> {
        self.entries()
            .await
            .into_iter()
            .find(|e| e.client_id == client_id)
    }

    /// Discord user ids of every live engine's bot. HQ uses this as the set of
    /// "our" bots when reading Discord's voice state.
    pub async fn client_ids(&self) -> Vec<String> {
        self.entries()
            .await
            .into_iter()
            .map(|e| e.client_id)
            .filter(|id| !id.is_empty())
            .collect()
    }
}

/// Deterministic rendezvous (highest-random-weight) rank of an engine for a
/// session.
///
/// The rank is a pure function of `(sink_id, guild_id, channel_id)` and the
/// engine set, so the same session always resolves to the same engine across
/// HQ restarts, retries and races. That is what makes a repeated Join
/// idempotent — the engine recognises the session and answers `AlreadyJoined`
/// — and what stops a second physical bot from joining a channel somebody
/// already occupies.
///
/// FxHasher is used because it is seedless and therefore stable across
/// processes, unlike the randomly-seeded default `HashMap` hasher.
fn hrw_weight(sink_id: &str, session: &SessionInfo) -> u64 {
    let mut h = FxHasher::default();
    sink_id.hash(&mut h);
    u64::from(session.guild_id).hash(&mut h);
    u64::from(session.channel_id).hash(&mut h);
    h.finish()
}

/// Placement candidates for `session`, best first.
///
/// An engine is eligible when it (a) is permitted for the session's guild,
/// (b) is not already serving a *different* channel in that guild — Discord
/// allows a bot token one voice connection per guild — and (c) is still
/// heartbeating. `already_serving` is the bot Discord says is *in this very
/// channel*; it is moved to the front so a re-Join lands on the bot that is
/// already there and comes back as `AlreadyJoined` rather than allocating a
/// second one.
pub fn rank_engines(
    entries: &[AeEntry],
    session: &SessionInfo,
    occupied_client_ids: &HashSet<String>,
    already_serving: Option<&str>,
) -> Vec<AeEntry> {
    let mut eligible: Vec<AeEntry> = entries
        .iter()
        .filter(|e| e.is_permitted_for(&session.guild_id))
        .filter(|e| !occupied_client_ids.contains(&e.client_id))
        .cloned()
        .collect();

    // Descending weight; ties broken by sink_id so the order is total and
    // independent of the registry's iteration order.
    eligible.sort_by(|a, b| {
        hrw_weight(&b.sink_id, session)
            .cmp(&hrw_weight(&a.sink_id, session))
            .then_with(|| a.sink_id.cmp(&b.sink_id))
    });

    if let Some(client_id) = already_serving {
        if let Some(pos) = eligible.iter().position(|e| e.client_id == client_id) {
            let entry = eligible.remove(pos);
            eligible.insert(0, entry);
        }
    }

    eligible
}

/// Turn an engine's raw response into a typed result, keeping structured
/// failures (notably `Tap`) intact so the bot can localize them.
pub fn classify(
    response: AudioEngineCommandResponse,
) -> Result<AudioEngineCommandResponse, AeError> {
    match response {
        AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined) => {
            Err(AeError::AlreadyJoined)
        }
        AudioEngineCommandResponse::Error(AudioEngineError::NotJoined) => Err(AeError::NotJoined),
        AudioEngineCommandResponse::Error(AudioEngineError::PermissionDenied) => {
            Err(AeError::PermissionDenied)
        }
        AudioEngineCommandResponse::Error(AudioEngineError::Tap(t)) => Err(AeError::Tap(t)),
        AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg)) => {
            Err(AeError::Transport(msg))
        }
        ok => Ok(ok),
    }
}

/// Send one command to one engine.
pub async fn dispatch(
    entry: &AeEntry,
    request: AudioEngineCommandRequest,
) -> Result<AudioEngineCommandResponse, AeError> {
    AudioEngineRpcClient::execute(&entry.client, request)
        .await
        .map_err(|e| AeError::Transport(e.to_string()))
        .and_then(classify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hq_types::ChannelId;

    fn entry(sink_id: &str, client_id: &str, guild: u64) -> AeEntry {
        let mut allowed_guilds = HashSet::new();
        allowed_guilds.insert(GuildId::from(guild));
        AeEntry {
            sink_id: sink_id.to_string(),
            client_id: client_id.to_string(),
            addr: format!("http://{sink_id}:8090"),
            allowed_guilds,
            last_heartbeat: Instant::now(),
            client: HttpClientBuilder::default()
                .build("http://127.0.0.1:1")
                .unwrap(),
        }
    }

    fn session(guild: u64, channel: u64) -> SessionInfo {
        SessionInfo {
            guild_id: GuildId::from(guild),
            channel_id: ChannelId::from(channel),
        }
    }

    #[test]
    fn placement_is_deterministic_across_calls() {
        let entries = vec![
            entry("ae-0", "1", 1),
            entry("ae-1", "2", 1),
            entry("ae-2", "3", 1),
        ];
        let s = session(1, 100);
        let a: Vec<_> = rank_engines(&entries, &s, &HashSet::new(), None)
            .into_iter()
            .map(|e| e.sink_id)
            .collect();
        let b: Vec<_> = rank_engines(&entries, &s, &HashSet::new(), None)
            .into_iter()
            .map(|e| e.sink_id)
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn placement_ignores_input_order() {
        let s = session(7, 42);
        let fwd = vec![
            entry("ae-0", "1", 7),
            entry("ae-1", "2", 7),
            entry("ae-2", "3", 7),
        ];
        let mut rev = fwd.clone();
        rev.reverse();
        let a: Vec<_> = rank_engines(&fwd, &s, &HashSet::new(), None)
            .into_iter()
            .map(|e| e.sink_id)
            .collect();
        let b: Vec<_> = rank_engines(&rev, &s, &HashSet::new(), None)
            .into_iter()
            .map(|e| e.sink_id)
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn placement_reuses_the_bot_already_in_the_channel() {
        let entries = vec![
            entry("ae-0", "1", 1),
            entry("ae-1", "2", 1),
            entry("ae-2", "3", 1),
        ];
        let s = session(1, 100);
        let base = rank_engines(&entries, &s, &HashSet::new(), None);
        // Pick whichever engine is NOT already first, so "existing wins" is a
        // real assertion rather than a coincidence.
        let other = base
            .iter()
            .find(|e| e.sink_id != base[0].sink_id)
            .unwrap()
            .client_id
            .clone();
        let ranked = rank_engines(&entries, &s, &HashSet::new(), Some(&other));
        assert_eq!(ranked[0].client_id, other);
    }

    #[test]
    fn placement_excludes_engines_busy_in_another_channel() {
        let entries = vec![entry("ae-0", "1", 1), entry("ae-1", "2", 1)];
        let s = session(1, 100);
        let busy: HashSet<String> = ["2".to_string()].into_iter().collect();
        let ranked = rank_engines(&entries, &s, &busy, None);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].client_id, "1");
    }

    #[test]
    fn placement_requires_guild_permission() {
        let entries = vec![entry("ae-0", "1", 999)];
        let ranked = rank_engines(&entries, &session(1, 100), &HashSet::new(), None);
        assert!(ranked.is_empty());
    }

    #[test]
    fn stale_engines_are_not_candidates() {
        let mut e = entry("ae-0", "1", 1);
        e.last_heartbeat = Instant::now() - HEARTBEAT_TTL - Duration::from_secs(1);
        assert!(!e.is_fresh());
    }
}
