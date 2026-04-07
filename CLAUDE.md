# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Zako3?

Zako3 is a community-driven Discord audio bot combining music and TTS functionality. Its core concept is **Taps** — community-hosted audio sources (TTS voices, music streams, etc.) that users can browse and select. Taps are third-party servers that connect to Zako3's infrastructure via the Zakofish protocol and gRPC.

## Build, Lint, and Test Commands

### Rust
```sh
cargo build                                                  # all packages
cargo build --package <pkg_name>                             # one package
cargo test --workspace                                       # all tests
cargo test --package <pkg_name> <test_fn> -- --nocapture    # single test (use --nocapture for tracing output)
cargo clippy --workspace --all-targets -- -D warnings        # lint
cargo fmt --all                                              # format
cargo fmt --all -- --check                                   # check format
```

### TypeScript (pnpm)
```sh
pnpm install
pnpm -r build                              # all packages
pnpm --filter <pkg_name> build             # one package
pnpm -r lint
pnpm --filter <pkg_name> test -t "<pattern>"   # single test
pnpm --filter web typecheck
```

Web-specific (`web/`): `dev`, `build`, `lint`, `lint:fix`, `format`, `test`, `test:run`, `storybook`.

### Database migrations
Migrations in `hq/core/migrations/` run automatically on HQ boot. To apply manually: `sqlx migrate run` (requires `DATABASE_URL`).

## Architecture

This is a polyglot monorepo. Rust uses Cargo workspaces; TypeScript uses pnpm workspaces.

### Services and how they connect

```
Web (React/Vite, :5173 dev)
  └─► HQ Backend (Axum REST, :8081)  /api/v1/...
  └─► HQ RPC (jsonrpsee, :3001)      internal, token-authenticated

HQ Boot (hq/boot) orchestrates three concurrent processes:
  1. HQ Backend  – REST API + OpenAPI (Swagger at /swagger-ui)
  2. HQ RPC      – JSON-RPC server for internal service-to-service calls
  3. HQ Bot      – Discord bot (serenity)

TapHub (taphub/) – manages Tap lifecycle and sessions
  └─► gRPC server (taphub/transport/server)
  └─► Zakofish protocol (zakofish/) for audio routing between Taps and the Audio Engine

Audio Engine (audio-engine/) – audio mixing/processing, gRPC-controlled
```

### Key Rust crates

| Crate | Purpose |
|---|---|
| `hq/boot` | Entry point; starts backend, RPC server, and Discord bot together |
| `hq/backend` | Axum handlers, OpenAPI schema, RPC method definitions |
| `hq/core` | Business logic: services, repositories, DB migrations, config |
| `hq/bot` | Discord bot commands (serenity) |
| `taphub/core` | Tap connection management, authentication, session state |
| `taphub/transport/server` | gRPC server for Tap communication |
| `zakofish` | Audio routing protocol built on protofish2; connects Taps to the Hub |
| `types` | Shared Rust types; `codegen` binary emits TypeScript Zod schemas |
| `crates/states` | Redis-backed state services (`TapHubStateService`, `TapMetricsStateService`, `UserSettingsStateService`) |
| `crates/preload-cache` | Audio preloading |
| `worker/emoji-matcher` | Emoji processing |
| `worker/metrics-sync` | Metrics synchronization |

### HQ Core internal structure

- **`service/`** – one service file per domain (`auth`, `tap`, `api_key`, `audit_log`, `verification`, `user_settings`, `notification`). All services are held in the `Service` struct and injected into Axum state via `Arc<Service>`.
- **`repo/`** – repository traits + `Pg*` implementations using raw sqlx queries (no ORM). Each repo corresponds to a DB table.
- **`migrations/`** – numbered SQL migration files applied via sqlx on startup.
- **`error.rs`** – `CoreError` enum; `serde_json::Error`, `sqlx::Error`, etc. all `From`-convert into it so `?` propagates cleanly.

### Caching pattern (Redis)

`crates/states` owns the `CacheRepository` trait and `RedisCacheRepository`. Service-level cache wrappers (e.g. `UserSettingsStateService`) hold a `CacheRepositoryRef` and implement cache-aside: check cache → load from DB on miss → populate cache. Cache keys follow `{entity}:{id}` patterns (e.g. `user_settings:{user_id}`, `tap:{tap_id}`).

### Frontend structure (`web/src/`)

- **`features/`** – feature-first layout. Each feature exports its components, hooks, API functions, and types from an `index.ts` barrel.
- **`lib/api-client.ts`** – thin `ApiClient` class using `fetch`; reads JWT from localStorage and attaches `Authorization: Bearer` header. All API calls use `apiCall()` from `api-helpers.ts` to unwrap `ApiResponse<T>` or throw.
- **`app/query-client.ts`** – shared TanStack Query `queryClient` instance.
- State: Zustand for global auth state; TanStack Query for server state.

### Type generation

`types/src/hq/` defines Rust structs/enums with `#[derive(Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]`. The `codegen` binary in `types/` emits Zod schemas consumed by the web and other TS packages. Run after changing shared types.

## Code Style

### Rust
- No `.unwrap()` or `.expect()` in non-test production code — propagate with `?`.
- Use `tracing::info!` / `tracing::error!` etc., not `println!`.
- `thiserror` for library errors, `anyhow` for application-level handling.
- Blocking work goes in `tokio::task::spawn_blocking`.

### TypeScript
- Strict TypeScript; no `any` — use `unknown` + type narrowing if truly dynamic.
- Prefer `interface` for extensible object shapes; `type` for unions/intersections.
- Named exports only (no default exports).
- Use `pnpm`, never `npm` or `yarn`.

## Adding new API endpoints

1. Add handler function in `hq/backend/src/handlers/<module>.rs` with `#[utoipa::path(...)]`.
2. Register the route in `hq/backend/src/lib.rs` (`app()` function).
3. Add the handler path to the `#[openapi(paths(...))]` macro and any new types to `components(schemas(...))`.
4. If new types are needed in OpenAPI schemas, derive `ToSchema` on them in `types/src/hq/`.

## Adding new migrations

Create `hq/core/migrations/<timestamp>_<description>.sql`. Timestamps must be strictly ascending. Migrations run on next boot.
