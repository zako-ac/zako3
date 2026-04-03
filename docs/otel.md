# OpenTelemetry Observability Plan for Zako3

This document outlines the comprehensive strategy for implementing OpenTelemetry (OTel) across the Zako3 ecosystem (`hq`, `taphub`, and `audio-engine`). It follows the RED (Rate, Errors, Duration) and USE (Utilization, Saturation, Errors) methods.

## 1. The Distributed Trace Lifecycle (Context Propagation)

To ensure end-to-end visibility across microservices, trace context must be propagated through the entire stack.
When a user interaction occurs (e.g., clicking a button in the Web UI or running a Discord Command):
1. **HQ** starts a Root Span (e.g., `[POST /api/taps/play]`).
2. **HQ** attaches trace IDs to the gRPC/TCP transport payload when calling **TapHub**.
3. **TapHub** extracts the trace ID and creates a child span (`[RPC Call: PlayAudio]`).
4. **Audio Engine** extracts the trace ID from TapHub and creates a child span (`[Audio Session: Start Track]`).

This produces a single waterfall chart in Jaeger/Zipkin showing exactly how long the HTTP request took, how much of that was TapHub network latency, and how long the audio engine took to decode the first frame.

---

## 2. Component Observability

### HQ (API Backend, Bot & Core)

**Traces (Spans):**
*   **HTTP Requests:** Span for every incoming API request via Axum (`http.method`, `http.route`, `http.status_code`, `http.client_ip`).
*   **Discord Interactions:** Span for every bot command invocation (`command.name`, `discord.guild.id`, `discord.user.id`).
*   **Database Queries:** Child spans for SQLx queries (`db.system=postgresql`, `db.statement=SELECT ...`, `duration`).
*   **External APIs:** Spans for outgoing requests (e.g., Discord API calls, OAuth providers).

**Metrics:**
*   `hq_http_requests_total` (Counter): Grouped by `route`, `method`, and `status_code`.
*   `hq_http_request_duration_seconds` (Histogram): API latency.
*   `hq_discord_commands_total` (Counter): Grouped by `command_name` and `result` (success/error).
*   `hq_discord_events_total` (Counter): Grouped by event type (e.g., `voice_state_update`, `message_create`).
*   `hq_db_pool_active_connections` (Gauge): Current active SQLx connections.
*   `hq_db_query_duration_seconds` (Histogram): Database query latency.

**Logs (Events):**
*   **INFO:** Bot connection/resumption to Discord Gateway, server startup, scheduled task execution.
*   **WARN:** Rate limits hit (Discord API), unauthorized API access attempts.
*   **ERROR:** Unhandled panics, database connection loss, invalid data payloads received.

### TapHub (Transport & RPC Layer)

**Traces (Spans):**
*   **RPC Send:** Span for the client sending a payload (`rpc.method`, `rpc.service`).
*   **RPC Receive:** Span for the server handling the payload.
*   **Pub/Sub Broadcast:** Span for broadcasting events to multiple connected nodes.

**Metrics:**
*   `taphub_rpc_requests_total` (Counter): Grouped by `method` and `status`.
*   `taphub_rpc_duration_seconds` (Histogram): Latency of the transport layer (network + serialization).
*   `taphub_active_connections` (Gauge): Number of currently connected clients/nodes.
*   `taphub_bytes_transferred_total` (Counter): Grouped by `direction` (tx/rx) to monitor bandwidth.
*   `taphub_serialization_errors_total` (Counter): Deserialization/parsing failures.

**Logs (Events):**
*   **INFO:** Node connected/disconnected, handshake success.
*   **WARN:** High latency detected, dropped packets (if using UDP/jitter buffers).
*   **ERROR:** Connection drops, protocol version mismatches, TLS handshake failures.

### Audio Engine (Controller, Core & Processing)

**Traces (Spans):**
*   **Session Lifecycle:** Long-running span representing an active Voice connection (`discord.guild.id`, `discord.channel.id`).
*   **Track Lifecycle:** Span for an individual track (`track.url`, `track.source`). *Child spans:*
    *   `Preload / Probe Format`
    *   `Decode Frames`
*   **Mixer Loop:** (Sampled rarely, e.g., 1%) Span for the 20ms mixer tick to identify CPU bottlenecks.

**Metrics:**
*   **Session Metrics:**
    *   `audio_session_active_total` (Gauge): Active guild audio sessions.
    *   `audio_track_lifecycle_total` (Counter): Grouped by `event` (queued, start, end, stop, skip) and `queue_name`.
*   **Performance / Mixer Metrics:**
    *   `audio_mixer_processing_duration_seconds` (Histogram): Time taken for a 20ms mixer iteration. *(Critical for audio glitch detection)*.
    *   `audio_mixer_active_sources` (Gauge): Current concurrent playing tracks.
    *   `audio_mixer_buffer_depth_samples` (Gauge): Buffer health.
    *   `audio_mixer_underruns_total` (Counter): Mixer starvation (audio glitches).
*   **Decoder / Stream Metrics:**
    *   `audio_decode_errors_total` (Counter): Grouped by `error_type` (io, codec, no_track).
    *   `audio_decoder_stalls_total` (Counter): Decoder buffer full.
    *   `audio_stream_underruns_total` (Counter): Output stream starvation.
*   **TapHub Integration:**
    *   `audio_taphub_request_duration_seconds` (Histogram).

**Logs (Events):**
*   **INFO:** Voice channel joined/left, track started/finished.
*   **WARN:** Audio underrun detected (rate-limited logging), track skipped due to slow decoding.
*   **ERROR:** Discord Voice Gateway disconnects, FFmpeg/Opus fatal errors, inability to stream audio packets.

---

## 3. Implementation Strategy & Semantic Conventions

To make querying this data easy in Grafana/Prometheus/Jaeger, we will standardize our attributes (labels) across all three codebases:
*   `service.name`: `hq`, `taphub`, `audio-engine`.
*   `service.version`: Automatically injected from `Cargo.toml`.
*   `discord.guild.id`: Attached to traces/metrics whenever the context relates to a specific Discord server.
*   `discord.user.id`: Attached when an action is triggered by a specific user.

### Phased Integration Plan

1. **Extract Telemetry Crate:** Move `audio-engine/telemetry` to a workspace-level crate (e.g., `zako3-telemetry`) so it can be shared across all projects.
2. **Integrate into HQ:** Add the shared telemetry crate to `hq/boot`, configure it to read environment variables (OTLP endpoint, metrics port), and add Axum tracing middleware for HTTP requests.
3. **Integrate into TapHub:** Add tracing instrumentation to Taphub's core RPC methods and ensure OTel Trace Context is propagated across the transport layer.
4. **Standardize Metrics:** Transition the existing raw Prometheus metrics in `audio-engine` to use the vendor-agnostic OpenTelemetry Metrics API, and implement the same for HQ and TapHub.
