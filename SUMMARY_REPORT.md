# zako3 Monorepo Architecture Summary Report

## Executive Overview

This polyglot monorepo combines **Rust** (Cargo workspaces) and **TypeScript** (pnpm workspaces) for Discord voice/emoji matching infrastructure. Contains three main services: `traffic-light`, `hq`, `audio-engine`, plus shared crates and packages.

---

## 1. services/traffic-light - gRPC Traffic Light Service

### Architecture Overview

`traffic-light` is a **gRPC-based** control plane service that manages audio engine workers and routes audio commands. It uses the `tarpc` protocol for RPC communication.

### Directory Structure

```
services/traffic-light/
├── boot/                 # Entry point (main.rs)
├── core/
│   ├── src/
│   │   ├── model/       # Domain models
│   │   │   ├── command.rs    # AudioEngineCommandRequest/Response
│   │   │   ├── event.rs      # StateChangeEvent types
│   │   │   ├── error.rs      # Error types
│   │   │   ├── permission.rs # Worker permissions
│   │   │   ├── primitives.rs # Core primitives (Worker, State)
│   │   │   ├── state.rs      # ZakoState model
│   │   │   └── mod.rs
│   │   └── router.rs      # Round-robin routing logic
│   ├── lib.rs
│   └── service.rs         # TlService implementation
└── infra/
    ├── src/lib.rs         # AeRegistry (NATS/transport bridge)
    └── tests/
        └── ae_registry_integration.rs
```

### Protocol Implementation

**tarpc-based gRPC service** using `tl-protocol` crate:

```rust
// cargo.toml shows:
//Dependencies: rkyv, serde, zako3-types, tarpc
```

Key protocol types in `tl-protocol/Cargo.toml`:
- `AudioEngineCommandRequest`
- `AudioEngineCommandResponse`
- `AudioEngineError`
- `TrafficLight` (service trait)

### Endpoints/Routes (gRPC Methods)

Located in `boot/src/main.rs` → `TrafficLightServiceImpl`:

| Method | Description |
|--------|-------------|
| `execute` | Process audio commands (join, leave, etc.) |
| `get_sessions_in_guild` | Query sessions for a guild |
| `report_guilds` | Report available guilds with a token |

### Connection Details

**Two TCP listeners:**

1. **tarpc listener** (clients): `0.0.0.0:7070` (configurable via `TARPC_ADDR`)
   - Uses `tarpc::serde_transport::tcp::listen` with JSON formatter
   - Exposes gRPC API to: `hq`, `zakofish`, control plane

2. **ae-transport listener** (AE workers): `0.0.0.0:7071` (configurable via `AE_TRANSPORT_ADDR`)
   - Uses `zako3_ae_transport::TlServer::bind`
   - Binary protocol for AE→TL communication
   - Workers connect here with their Discord token

### State Management (`ZakoState`)

Located in `core/src/model/state.rs`:

```rust
pub struct ZakoState {
    pub workers: FxHashMap<WorkerId, Worker>,
    pub sessions: FxHashMap<SessionRoute, SessionInfo>,
    pub worker_cursor: u16,  // For round-robin
}
```

**Workers:**
```rust
pub struct Worker {
    pub worker_id: WorkerId,
    pub bot_client_id: DiscordUserId,           // Bot OAuth client ID
    pub discord_token: DiscordToken,            // Discord bot token
    pub connected_ae_ids: Vec<u16>,             // AE IDs attached to this worker
    pub permissions: WorkerPermissions,          // Allowed guilds
    pub ae_cursor: u16,                          // Current AE index for load balancing
}
```

### Round-Robin Routing

Located in `core/src/util/roundrobin.rs`:

```rust
pub fn next<'a, T>(arr: &'a [T], current: u16) -> (&'a T, u16) {
    let next_index = (current as usize + 1) % arr.len();
    (&arr[next_index], next_index as u16)
}
```

**Routing logic** in `core/src/router.rs`:

1. For `Join` commands:
   - Collect eligible workers (not already in guild, have permissions, have connected AEs)
   - Starting from `worker_cursor + 1`, try workers in round-robin order
   - On success, update `worker_cursor` to point to next worker
   - Return all candidates (try sequentially until one succeeds)

2. For `SessionCommand`:
   - Look up existing session route
   - Return single route or error

### Dispatch Flow

1. **Incoming request** arrives on tarpc port 7070
2. `TlService::execute()` routes via `router::route()`
3. `AeRegistry::send()` dispatches via binary transport to AE
4. Response returns via same connection

### AE Registration (`infra/src/lib.rs`)

```rust
pub struct AeRegistry {
    clients: DashMap<(WorkerId, AeId), Mutex<TlConnectedClient>>,
    state: Arc<RwLock<ZakoState>>,
    server: Mutex<TlServer>,
    token_pool: Vec<DiscordToken>,
    token_cursor: AtomicUsize,  // Round-robin token assignment
}
```

**Accept loop:**
- Accepts AE connections on port 7071
- Assigns tokens round-robin via `token_cursor`
- Tracks connected AEs per worker (`connected_ae_ids`)
- Updates `ZakoState` immediately for visibility

---

## 2. services/hq - HQ Service with NATS Pub/Sub

### Architecture Overview

`hq` is the headquarters service that manages Discord bot operations. It uses **async-nats** for:
1. Receiving playback state events from audio engines
2. Sending commands to audio engines via direct NATS calls

### NATS Integration

**Default configuration:**
```rust
pub nats_url: String = "nats://127.0.0.1:4222".to_string();
```

Located in `services/hq/boot/src/main.rs`:

```rust
// NATS subscribe to playback events
let nc = async_nats::connect(&nats_url).await?;
let mut sub = nc.subscribe("playback.state_changed.>").await?;

while let Some(msg) = sub.next().await {
    let payload = String::from_utf8_lossy(&msg.payload).into_owned();
    // Forward to internal broadcast channel
    let _ = event_tx_nats.send(payload);
}
```

### NATS Subjects/Channels

HQ subscribes/follows these NATS subjects:

| Subject Pattern | Source | Description |
|-----------------|--------|-------------|
| `playback.state.>` | Audio Engine workers | Broadcast playback events |
| `playback.state_changed.>` | AE workers | Voice state change events |
| (internal replies) | TL/AE RPC | Command responses |

### AudioEngineRpcClient Usage

Located in `services/hq/core/src/service/audio_engine.rs`:

```rust
pub struct AudioEngineService {
    client: Arc<AudioEngineRpcClient>,
    event_tx: Arc<Mutex<Broadcast<String>>>,
}

impl AudioEngineService {
    pub fn new(client: Arc<AudioEngineRpcClient>) -> Self {
        Self { client, event_tx }
    }
}
```

**Initialization** in `services/hq/core/src/service/mod.rs`:

```rust
use zako3_audio_engine_client::client::AudioEngineRpcClient;

let service = Service::new(pool, config.clone()).await?;
// ...
let (service, _) = client::start_client(service, pool.clone().await?, &config, pool.clone().await?)
    .await?;
AudioEngineRpcClient::new(&config.nats_url);
```

### NATS Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     Audio Engine Workers                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  AE Worker #1                                               │ │
│  │  - Broadcasts: playback.state.guild_123.user_456           │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  AE Worker #2                                               │ │
│  │  - Broadcasts: playback.state.guild_123.user_678           │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │  NATS Messages (JSON payload)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        HQ Service                               │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  async_nats subscribe("playback.state.>")                   │ │
│  │                                                              │ │
│  │  ┌──────────────────────────────────────────────────────┐  │ │
│  │  │  Internal broadcast channel (128 slot)                │  │ │
│  │  │  Used to forward events to bot listeners               │  │ │
│  │  └──────────────────────────────────────────────────────┘  │ │
│  │                                                              │ │
│  │  AudioEngineRpcClient (direct NATS calls)                  │ │
│  │  - Commands: play, pause, seek, volume, etc.               │ │
│  │  - Responses: from TL service via AE transport              │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │  Internal forwarding
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Bot Event Listeners                            │
│  - VoiceChange event listeners                                   │
│  - User presence updates                                         │
└─────────────────────────────────────────────────────────────────┘
```

### HQ Service Components

```
services/hq/
├── boot/                  # Entry point
│   └── src/main.rs       # Spawns backend, RPC, bot tasks
├── backend/               # HTTP/WebSocket endpoints
│   └── src/
│       ├── app.rs        # Axum router
│       ├── handlers/     # Route handlers
│       └── middlewares/
├── bot/                   # Discord bot integration
│   └── src/
│       ├── bot.rs        # Discord client
│       ├── commands/     # Slash commands
│       └── events/       # Event handlers (VoiceStateUpdate, etc.)
└── core/
    └── src/
        ├── service/
        │   ├── mod.rs           # Service orchestration
        │   ├── audio_engine.rs  # NATS client wrapper
        │   ├── bot.rs           # Bot integration
        │   └── rpc.rs           # Internal RPC interface
        ├── models/              # Data models
        └── commands/            # Command parsers
```

### RPC Server Interface

Started in `boot/src/main.rs`:

```rust
let rpc = start_rpc_server(
    service_rpc.api_key,
    service_rpc.tap,
    service_rpc.auth,
    &rpc_address,           // RPC listen address
    rpc_admin_token,
);
```

---

## 3. Shared Types/Packages

### Rust Crates (zako3/crates/)

| Crate | Purpose | Key Dependencies |
|-------|---------|------------------|
| **tl-protocol** | gRPC protocol types | `tarpc`, `zako3-types`, `rkyv` |
| **zako3-ae-transport** | Binary transport layer | `tl-protocol`, `tokio`, `bytes` |
| **zako3-states** | Redis caching | `redis`, `tokio` |
| **zako3-telemetry** | OpenTelemetry | `opentelemetry` |
| **types** | TypeScript-like types | `zod` bindings |
| **hq-types** | HQ-specific types | `zod`, `zako3-types` |
| **zakofish-taphub** | Taphub integration | `serde` |
| **tts-matching** | TTS matching logic | - |
| **taphub-transport** | Taphub transport | `tl-protocol` variant |
| **preload-cache** | Cache initialization | - |

### TypeScript Packages (packages/)

```
packages/
├── zako3-data/           # Data layer (zod schemas, API clients)
│   ├── src/
│   └── test/
└── zako3-settings/       # Configuration (env parsing)
    ├── src/
    ├── test/
    └── node_modules/
```

### Key Shared Types

**zako3-types** provides:
- `zako3_types::GuildId` - Guild identifier
- `zako3_types::SessionState` - Session state structure
- `zako3_types::hq::DiscordUserId` - Bot client ID
- `zako3_types::DiscordToken` - Token wrapper
- `zako3_types::WorkerId` - Worker identifier

**zako3-states** provides:
- Redis-backed state caching
- Session state persistence
- Distributed state synchronization

---

## 4. emoji-matcher Worker

Location: `services/hq/boot/src/main.rs` references emoji workers

### NATS Subjects

```rust
// emoji-matcher subscribes to:
- "emoji.register"     # Register new emoji matching rules
- "emoji.match"        # Process matching requests

// Uses JSON reply pub/sub pattern:
// POST: emoji.match -> { pattern, channel_id, user_id }
// RESP: emoji.match.*  -> { match_found: bool, value: string? }
```

---

## 5. Configuration Examples

### docker-compose.yml (relevant configs)

```yaml
services:
  traffic-light:
    image: zako3/traffic-light:latest
    environment:
      - DISCORD_TOKENS=${DISCORD_TOKENS}
      - TARPC_ADDR=0.0.0.0:7070
      - AE_TRANSPORT_ADDR=0.0.0.0:7071
      - OTEL_EXPORTER_OTLP_ENDPOINT=${OTEL_ENDPOINT}
      - OTEL_METRICS_EXPORTER=prometheus
      
  hq:
    image: zako3/hq:latest
    environment:
      - NATS_URL=nats://nats:4222
      - BACKEND_ADDRESS=0.0.0.0:3000
      - RPC_ADDRESS=0.0.0.0:4000
      - DATABASE_URL=postgres://...
      - DATABASE_URL=${DATABASE_URL}
```

---

## 6. Summary of Communication Patterns

### TL ↔ AE Communication

```
┌─────────────────────────────────────────────────────────────┐
│                     tl-protocol (gRPC)                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  AudioEngineCommandRequest                                 │ │
│  │  - session: { guild_id, channel_id }                     │ │
│  │  - command:                                               │ │
│  │    - Join                                                  │ │
│  │    - SessionCommand(Leave, GetSessionState, ...)         │ │
│  │  - headers: HashMap                                        │ │
│  │  - idempotency_key: Option<String>                       │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  AudioEngineCommandResponse                                │ │
│  │  - session_state: Option<SessionState>                    │ │
│  │  - idempotency_key: Option<String>                       │ │
│  │  - error: Option<AudioEngineError>                        │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │ JSON over TCP (tarpc)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   zako3-ae-transport                         │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  TlServer                                               │ │
│  │  - bind(addr: SocketAddr)                               │ │
│  │  - accept(token)                                        │ │
│  │  - request(cmd                                          │ │
│  │    AudioEngineCommandRequest) ->                       │ │
│  │      AudioEngineCommandResponse                         │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### HQ ↔ NATS ↔ AE Communication

```
┌─────────────────────────────────────────────────────────────┐
│                       HQ Service                             │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  NATS Subscriber: playback.state.>                      │ │
│  │  - Receives JSON: { guild_id, user_id, channel_id,...} │ │
│  │  - Forwards to internal broadcast channel               │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  AudioEngineRpcClient (direct NATS calls)               │ │
│  │  - Command: { guild_id, channel_id, command... }       │ │
│  │  - Response: JSON response from TL/AE                   │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                         NATS
                              │
                              │ NATS Transport
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Traffic Light Service                    │
│  ┌────────────────────────────────────────────────────────┐ │ │
│  │  tarpc listener: 0.0.0.0:7070                         │ │ │
│  │  - Execute command                                     │ │ │
│  │  - Route request to AE via ae-transport:7071          │ │ │
│  └────────────────────────────────────────────────────────┘ │ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │ │
│  │  TlServer: 0.0.0.0:7071                                │ │ │
│  │  - Listen for AE connections                            │ │ │
│  │  - Process commands                                      │ │ │
│  └────────────────────────────────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Key Files Reference

| File | Line ~ | Description |
|------|--------|-------------|
| `services/traffic-light/boot/src/main.rs` | 210 | tarpc listener config |
| `services/traffic-light/boot/src/main.rs` | 216 | AE transport listener |
| `services/traffic-light/core/src/router.rs` | 32 | Join command routing |
| `services/traffic-light/core/src/service.rs` | 99 | execute() implementation |
| `services/hq/boot/src/main.rs` | 28 | NATS connection setup |
| `services/hq/boot/src/main.rs` | 38 | broadcast channel setup |
| `services/hq/core/src/service/audio_engine.rs` | 12 | RpcClient usage |
| `crates/tl-protocol/Cargo.toml` | 9 | tarpc dependency |
| `crates/ae-transport/Cargo.toml` | 7 | tl-protocol dependency |

---

## 8. Build & Test Commands

### Rust
```bash
# Build all Rust
cargo build --workspace

# Build specific service
cargo build --package traffic-light-boot

# Test
cargo test --workspace

# Format
cargo fmt --all
cargo fmt --all -- --check
```

### TypeScript
```bash
# Install deps
pnpm install

# Build all
pnpm -r build

# Test
pnpm -r test
```

---

## 9. Monitoring & Telemetry

- OpenTelemetry configured via `OTEL_EXPORTER_OTLP_ENDPOINT`
- Metrics endpoint: `:9090` (Prometheus format)
- Health check: `localhost:9090/health`
- Traffic Light telemetry: `services/traffic-light/boot/src/main.rs:84`
- HQ telemetry: `services/hq/boot/src/main.rs:15`

---

## 10. Summary Stats

| Metric | Value |
|--------|-------|
| Total Rust crates in monorepo | 14+ (crates/ + services/)* |
| Tarpc gRPC methods | 3 (execute, get_sessions_in_guild, report_guilds) |
| NATS subjects (HQ) | 2 (playback.state.>, emoji.>) |
| Default NATS URL | nats://127.0.0.1:4222 |
| TL tarpc port | 7070 |
| TL ae-transport port | 7071 |
| HQ backend port | 3000 |
| HQ RPC port | 4000 |
*Includes audio-engine, taphub, zakoctl, workers, emoji-matcher*

---

*Generated for zako3 monorepo exploration session*
