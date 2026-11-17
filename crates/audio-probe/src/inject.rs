//! Synthetic stall injection.
//!
//! A stall detector that has never fired is not a verified detector. This module
//! is how the self-test makes it fire on purpose: for a named stream (or `*`) at
//! a named stage, every frame is discarded for the configured duration, so the
//! path downstream of that stage goes quiet exactly as a real break would.
//!
//! Two stages, because the two answers worth distinguishing sit either side of
//! the socket:
//!
//! - [`Stage::EnginePump`] drops a frame the engine has already read and counted
//!   off the socket. The socket counter keeps advancing and the buffer counter
//!   does not — the signature of an engine-internal break.
//! - [`Stage::TaphubSend`] drops a frame at taphub before it reaches the
//!   transport. The engine's socket counter goes quiet while taphub's own
//!   send timestamp stays fresh — the signature of a path loss.
//!
//! Off by default. It is enabled for an investigation window with one env var:
//!
//! ```text
//! ZAKO_STALL_INJECT=<stream-pattern>:<engine-pump|taphub-send>:<seconds>
//! ```
//!
//! A pattern of `*` matches every stream. The injection is time-boxed: when the
//! window closes the rule expires and the process behaves normally again, so a
//! forgotten flag cannot keep discarding audio.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Where a frame is discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// After the engine read and counted the frame, before the jitter buffer.
    EnginePump,
    /// At taphub, before the frame is handed to the transport.
    TaphubSend,
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::EnginePump => "engine_pump",
            Stage::TaphubSend => "taphub_send",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "engine-pump" | "engine_pump" | "pump" => Some(Stage::EnginePump),
            "taphub-send" | "taphub_send" | "send" => Some(Stage::TaphubSend),
            _ => None,
        }
    }
}

struct Rule {
    pattern: String,
    stage: Stage,
    until: Instant,
}

/// Set while at least one rule exists, so the hot path can skip the lock.
static ACTIVE: AtomicBool = AtomicBool::new(false);
static RULES: LazyLock<Mutex<Vec<Rule>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static ENV_INIT: LazyLock<()> = LazyLock::new(|| {
    init_from_env();
});

/// Arm an injection rule.
pub fn inject(stream: &str, stage: Stage, duration: Duration) {
    if duration.is_zero() {
        return;
    }
    let mut rules = RULES.lock().expect("injection lock");
    rules.retain(|r| r.until > Instant::now());
    rules.push(Rule {
        pattern: stream.to_string(),
        stage,
        until: Instant::now() + duration,
    });
    ACTIVE.store(true, Ordering::Relaxed);
    tracing::warn!(
        stream_pattern = %stream,
        stage = stage.as_str(),
        duration_ms = duration.as_millis() as u64,
        "synthetic stall injection armed"
    );
}

/// Whether a frame on `stream` should be discarded at `stage` right now.
pub fn dropping(stream: &str, stage: Stage) -> bool {
    LazyLock::force(&ENV_INIT);

    if !ACTIVE.load(Ordering::Relaxed) {
        return false;
    }

    let now = Instant::now();
    let mut rules = RULES.lock().expect("injection lock");
    rules.retain(|r| r.until > now);
    if rules.is_empty() {
        ACTIVE.store(false, Ordering::Relaxed);
        return false;
    }

    rules
        .iter()
        .any(|r| r.stage == stage && (r.pattern == "*" || stream.contains(&r.pattern)))
}

/// Drop every rule.
pub fn clear() {
    RULES.lock().expect("injection lock").clear();
    ACTIVE.store(false, Ordering::Relaxed);
}

/// Parse `pattern:stage:seconds`.
pub fn parse_spec(spec: &str) -> Option<(String, Stage, Duration)> {
    let mut parts = spec.split(':');
    let pattern = parts.next()?.trim();
    let stage = Stage::parse(parts.next()?)?;
    let secs: f64 = parts.next()?.trim().parse().ok()?;
    if pattern.is_empty() || secs <= 0.0 {
        return None;
    }
    Some((pattern.to_string(), stage, Duration::from_secs_f64(secs)))
}

/// Arm the injection described by `ZAKO_STALL_INJECT`, if any.
///
/// Returns the spec that was applied.
pub fn init_from_env() -> Option<String> {
    let spec = std::env::var("ZAKO_STALL_INJECT").ok()?;
    let spec = spec.trim().to_string();
    if spec.is_empty() {
        return None;
    }
    match parse_spec(&spec) {
        Some((pattern, stage, duration)) => {
            inject(&pattern, stage, duration);
            tracing::warn!(
                spec = %spec,
                "ZAKO_STALL_INJECT is set: discarding frames on purpose for the investigation window"
            );
            Some(spec)
        }
        None => {
            tracing::error!(
                spec = %spec,
                "ZAKO_STALL_INJECT is malformed (want <stream-pattern>:<engine-pump|taphub-send>:<seconds>); no injection armed"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The injection registry is process-global, so tests that arm and clear it
    /// must not run at the same time.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_a_spec() {
        let (pattern, stage, duration) = parse_spec("tts-123:engine-pump:2.5").expect("parses");
        assert_eq!(pattern, "tts-123");
        assert_eq!(stage, Stage::EnginePump);
        assert_eq!(duration, Duration::from_millis(2500));
    }

    #[test]
    fn rejects_a_bad_spec() {
        assert!(parse_spec("").is_none());
        assert!(parse_spec("a:nowhere:1").is_none());
        assert!(parse_spec("a:send:0").is_none());
        assert!(parse_spec("a:send").is_none());
    }

    #[test]
    fn injection_matches_stream_and_stage() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear();
        inject("guild-42", Stage::EnginePump, Duration::from_secs(30));
        assert!(dropping("guild-42-stream", Stage::EnginePump));
        assert!(!dropping("guild-42-stream", Stage::TaphubSend));
        assert!(!dropping("guild-43-stream", Stage::EnginePump));
        clear();
        assert!(!dropping("guild-42-stream", Stage::EnginePump));
    }

    #[test]
    fn wildcard_matches_every_stream() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear();
        inject("*", Stage::TaphubSend, Duration::from_secs(30));
        assert!(dropping("anything", Stage::TaphubSend));
        clear();
    }
}
