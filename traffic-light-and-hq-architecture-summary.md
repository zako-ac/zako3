# Traffic-Light and HQ Service Architecture Summary

This document summarizes the architecture of the `services/traffic-light` and `services/hq` services, focusing on their protocols, NATS usage patterns, and shared dependencies for migration planning.

---

## 1. services/traffic-light Service

### Overview
**services/traffic-light** is a traffic-light service that routes audio commands to Audio Engine (AE) workers using round-robin load balancing.

### Connection Protocol
- **Type**: gRPC over TCP
- **Implementation**: Uses `tarpc` library
- **Transport**: Binary protocol via `zako3-ae-transport` crate

### API Endpoints/Methods

The `TrafficLight` gRPC service provides 3 main methods:

```rust
#[tarpc::service]
pub trait TrafficLight {
    async fn execute(request: AudioEngineCommandRequest) -> AudioEngineCommandResponse;
    async fn get_sessions_in_guild(guild_id: GuildId) -> Vec<SessionState>;
    async fn report_guilds(token: String, guilds: Vec<GuildId>);
}
```

### Routing Algorithm
- Uses **round-robin** distribution across workers
- Maintains state in `ZakoState`:
  - `workers`: Map of worker_id → Worker configuration
  - `sessions`: Map of Worker/AE → SessionInfo
  - `worker_cursor`: Next worker to try
  - `ae_cursor`: Next AE on that worker to use

### Request Flow
```
1. Client sends AudioEngineCommandRequest
2. Router.finds eligible workers for the guild (permission checks)
3. Builds candidate routes in round-robin order
4. Tries candidates sequentially until one succeeds
5. Updates state with successful routing
6. Sends command via dispatcher to specified worker/AE
```

### State Management
- `Worker` struct:
  - `worker_id`: Worker identifier
  - `bot_client_id`: Discord bot account ID
  - `discord_token`: OAuth token for guild access
  - `connected_ae_ids`: List of AE identifiers (audio engines on this worker)
  - `permissions`: Worker permission set
  - `ae_cursor`: Next AE index to use

- `SessionRoute`: `(WorkerId, AeId)` tuple
- `SessionInfo`: Guild/channel pairing

---

## 2. services/hq Service - NATS Usage

### Overview
**services/hq** is the main orchestration service that integrates Discord bot functionality with audio playback. It uses NATS for two purposes:

1. **Pub/Sub for playback state events** (broadcast model)
2. **Direct RPC calls** to traffic-light (request/response)

### NATS Configuration
```rust
pub nats_url: String, // Default: "nats://127.0.0.1:4222"
```

Set via `NATS_URL` environment variable.

### NATS Client Setup (boot/src/main.rs)

#### 1. Playback State Event Subscription
```rust
let mut sub = nc.subscribe("playback.state.>").await;
while let Some(msg) = sub.next().await {
    let payload = String::from_utf8_lossy(&msg.payload);
    // Forward to internal broadcast channel
    let _ = event_tx_nats.send(payload);
}
```

- **Subject Pattern**: `playback.state.>`
- **Purpose**: Capture any `playback.state.*` events from audio engines
- **Action**: Convert to string payload and forward

#### 2. Direct RPC Calls to Traffic-Light
```rust
let audio_engine = Arc::new(
    AudioEngineRpcClient::new(&config.nats_url)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?,
);
```

### emoji-matcher Worker NATS Usage

#### Registration Handler
```rust
pub const SUBJECT_EMOJI_REGISTER: &str = "emoji.register";
let mut subscriber = client.subscribe(SUBJECT_EMOJI_REGISTER).await?;
```

#### Match Handler
```rust
pub const SUBJECT_EMOJI_MATCH: &str = "emoji.match";
let mut subscriber = client.subscribe(SUBJECT_EMOJI_MATCH).await?;
```

#### Reply Pattern
- Supports **direct reply** (`message.reply`) for request/response
- Publishes reply to the original caller's response subject

---

### NATS Data Flow

#### Playback State Events
```
┌─────────────────┐     subscribe       ┌─────────────────┐
│  AE Workers     │ ──────────────────▶ │  hq-boot        │
│  (broadcast)    │   playback.state.   │                  │
└─────────────────┘                     │ 1. Subscribe     │
                                         │ 2. Forward      │
┌─────────────────┐                     │ 3. Broadcast     │
│  hq-backend     │ ◀────────────────── │ 4. Internal      │
│  (handlers)     │                     │    Channel       │
└─────────────────┘                     └─────────────────┘
```

#### Direct RPC Calls
```
┌─────────────────┐     request         ┌─────────────────┐
│  hq-backend     │ ──────────────────▶ │  traffic-light  │
│  (client)       │   command           │  (server)       │
└─────────────────┘                     │                  │
                                         │ execute()        │
┌─────────────────┐                     │ get_sessions()    │
│  external       │ ◀────────────────── │ report_guilds()   │
│  caller         │   response          │                  │
└─────────────────┘                     └─────────────────┘
```

---

## 3. Shared Types/Packages

### Key Shared Dependencies

| Package | Purpose | Used By |
|---------|-----|---------|
| `zako3-types` | Common TypeScript-like type generation (zod-based) | Both |
| `zako3-tls-protocol` | gRPC message types (tarpc) | traffic-light |
| `zako3-audio-engine-client` | NATS client wrapper | hq |
| `zako3-ae-transport` | Binary transport protocol | traffic-light |
| `zako3-states` | Redis cache layer | Both |
| `zako3-telemetry` | OpenTelemetry integration | Both |
| `tl-protocol` | Tarpc service definitions | traffic-light |

### Common Type Patterns

#### Timestamp/Audio Types
```rust
pub type QueueName = usize;      // Queue identifier
pub type TapName = usize;        // Audio tap identifier  
pub type TrackId = usize;        // Track identifier
pub type Volume = u8;            // 0-255
pub type ChannelId = u64;       // Discord channel
pub type GuildId = u64;         // Discord guild
pub type DiscordUserId = u64;   // Discord user
```

#### Request/Response Patterns
- **traffic-light**: Uses `AudioEngineCommandRequest`/`AudioEngineCommandResponse` with serde `Serialize(Deserialize)`
- **hq**: Uses `AudioEngineRequest`/`AudioEngineResponse` enums with discriminator fields (`method`/`type`)

---

## 4. Migration Planning Notes

### If migrating hq from NATS to traffic-light:

#### Current hq NATS Usage:
1. **Event-driven**: Subscription to `playback.state.>` for broadcast playback events
2. **Command-driven**: Direct calls to audio engines via `AudioEngineRpcClient`

#### Considerations:
1. **Event model mismatch**: NATS broadcast vs internal channels - requires state management
2. **Protocol difference**: hq uses JSON NATS, traffic-light uses binary gRPC
3. **Subjects to map**? 
   - `playback.state.*` → need to replace with alternative event mechanism
   - `emoji.register`/`emoji.match` → worker-specific, not hq concern

#### Recommendations:
1. **Keep NATS for events** unless you have a stateful pub/sub alternative
2. **Binary transport for commands** can replace direct NATS calls
3. **Shared crates** like `zako3-types` and `zako3-audio-engine-client` provide compatibility layer

### Protocol Comparison

| Aspect | NATS (hq) | gRPC (traffic-light) |
|--------|-----------|---------------------|
| Format | JSON | Binary (tarpc) |
| Connection | Pub/Sub + Direct | TCP Server |
| Pattern | Event + RPC | RPC-only |
| Discovery | NATS server registry | Manual binding |
| Latency | ~ms | <ms (over same wire) |
