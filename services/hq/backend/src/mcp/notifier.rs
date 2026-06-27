//! Server-initiated playback notifications over MCP's SSE channel.
//!
//! Subscribes to the playback `event_tx` broadcast and, on a
//! `PlaybackEvent::PlaybackChanged`, forwards a JSON-RPC notification string to
//! the MCP SSE broadcast channel, which `mcp_sse` fans out to every connected
//! client. (Per the user's decision the existing `PlaybackChanged` event is
//! reused as the "playback end" signal.)

use hq_core::PlaybackEvent;
use mcpkit::protocol::{Message, Notification};
use tokio::sync::broadcast;

/// MCP notification method clients receive over `/mcp/sse`.
const PLAYBACK_METHOD: &str = "notifications/playback/changed";

/// Spawn the background task bridging playback events to MCP SSE clients.
pub fn spawn_playback_notifier(
    sse_tx: broadcast::Sender<String>,
    event_tx: broadcast::Sender<PlaybackEvent>,
) {
    tokio::spawn(async move {
        let mut rx = event_tx.subscribe();
        loop {
            match rx.recv().await {
                Ok(PlaybackEvent::PlaybackChanged) => {
                    let note = Notification::new(PLAYBACK_METHOD);
                    match serde_json::to_string(&Message::Notification(note)) {
                        // Ignore send errors (no SSE clients currently connected).
                        Ok(payload) => {
                            let _ = sse_tx.send(payload);
                        }
                        Err(e) => tracing::warn!(%e, "failed to serialize playback notification"),
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "playback notifier lagged");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
