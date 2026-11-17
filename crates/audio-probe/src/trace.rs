//! Carries the engine's track identity down to the transport.
//!
//! The stall line wants a track id, but the transport client is handed a request
//! and knows nothing about the session that made it. Threading a parameter
//! through the request (a shared, versioned wire type) to carry a local logging
//! detail would be the wrong trade: it changes a protocol to enrich a log.
//!
//! A task-local does it without touching either. The session layer scopes the
//! request with the track it is starting, and anything deeper in the same task —
//! including the transport client, before it spawns its own task — can read it.

use std::future::Future;

tokio::task_local! {
    static TRACE: StreamTrace;
}

/// Who asked for this stream, for the log line only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamTrace {
    pub track_id: Option<String>,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
}

impl StreamTrace {
    pub fn new(
        track_id: impl ToString,
        guild_id: impl ToString,
        channel_id: impl ToString,
    ) -> Self {
        Self {
            track_id: Some(track_id.to_string()),
            guild_id: Some(guild_id.to_string()),
            channel_id: Some(channel_id.to_string()),
        }
    }
}

/// Run `fut` with `trace` visible to [`current_trace`].
pub async fn with_trace<F: Future>(trace: StreamTrace, fut: F) -> F::Output {
    TRACE.scope(trace, fut).await
}

/// The trace scoped around the current task, if any.
pub fn current_trace() -> Option<StreamTrace> {
    TRACE.try_with(|t| t.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_trace_is_visible_inside_and_gone_outside() {
        assert!(current_trace().is_none());
        let seen = with_trace(StreamTrace::new("t1", "g1", "c1"), async {
            current_trace()
        })
        .await;
        assert_eq!(seen.expect("scoped").track_id.as_deref(), Some("t1"));
        assert!(current_trace().is_none());
    }
}
