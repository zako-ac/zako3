# OTLP Integration Guide

This document explains how to connect Zako3 services to an OpenTelemetry collector and visualize traces in OpenObserve (or any OTLP-compatible backend).

---

## What is instrumented

Every Zako3 service now exports:

| Signal | Backend |
|---|---|
| Distributed traces (spans) | OTLP gRPC → OpenObserve |
| Prometheus metrics | `/metrics` HTTP endpoint per service |
| Health check | `/health` HTTP endpoint per service |
| Structured logs | stdout (`RUST_LOG` controlled) |

### Services and their default metrics ports

| Service | `service.name` | Default metrics port |
|---|---|---|
| `hq-boot` | `hq` | `9091` |
| `taphub-core` | `taphub` | `9092` |
| `audio-engine-controller` | `audio-engine` (or `zako3-audio-engine`) | `9090` |
| `emoji-matcher` | `emoji-matcher` | own HTTP server (see its config) |
| `metrics-sync` | `metrics-sync` | none (batch job) |
| `cache` | `cache` | none (HTTP `/healthz` on `ZK_CACHE_BIND_ADDR`) |

---

## Environment variables

All services read these variables. Names are prefixed per service for TapHub; HQ and workers use un-prefixed names.

### HQ (`hq-boot`)

| Variable | Default | Description |
|---|---|---|
| `OTLP_ENDPOINT` | _(none)_ | gRPC endpoint for the OTLP collector, e.g. `http://localhost:5081`. Tracing is disabled if unset. |
| `METRICS_PORT` | `9091` | Port for `/health` and `/metrics`. |
| `RUST_LOG` | `info` | Log filter (e.g. `info,hq_core=debug`). |

### TapHub (`taphub-core`)

| Variable | Default | Description |
|---|---|---|
| `ZK_TH_OTLP_ENDPOINT` | _(none)_ | Same as above, but TapHub-specific. |
| `ZK_TH_METRICS_PORT` | `9092` | Port for `/health` and `/metrics`. |
| `RUST_LOG` | `info` | Log filter. |

### Audio Engine (`audio-engine-controller`)

| Variable | Default | Description |
|---|---|---|
| `OTLP_ENDPOINT` | _(none)_ | gRPC OTLP endpoint. |
| `METRICS_PORT` | `9090` | Port for `/health` and `/metrics`. |
| `RUST_LOG` | `debug` | Log filter. |

### Workers (`metrics-sync`, `cache`, `emoji-matcher`)

| Variable | Default | Description |
|---|---|---|
| `OTLP_ENDPOINT` | _(none)_ | gRPC OTLP endpoint. Traces disabled if unset. |
| `RUST_LOG` | `info` | Log filter. |

---

## Local stack: OpenObserve

OpenObserve is a single-binary observability backend that receives traces, metrics, and logs, with a built-in UI. No separate Grafana instance is needed.

Start the dev stack:

```sh
./scripts/dev-telemetry.sh
```

This starts OpenObserve via podman:
- Port `5080`: web UI + HTTP API
- Port `5081`: OTLP gRPC receiver

Open the UI at `http://localhost:5080` (login: `admin@localhost` / `admin123`).

---

## Connecting services

Set the OTLP endpoint in each service's `.env` (or shell environment) before starting:

```sh
# hq-boot
OTLP_ENDPOINT=http://localhost:5081

# taphub-core
ZK_TH_OTLP_ENDPOINT=http://localhost:5081

# audio-engine-controller
OTLP_ENDPOINT=http://localhost:5081
```

---

---

## What traces look like

### Tap lifecycle (`tap.connection`)

A single root span covers the entire lifetime of a connected Tap, from TLS handshake to clean disconnect. Child spans:

```
tap.connection (tap_id, connection_id, friendly_name, disconnect_reason)
  └─ tap.authenticate (auth.result: ok | rejected)
       └─ hq.rpc.authenticate_tap
  └─ [many] audio.request
       ├─ hq.rpc.get_tap
       ├─ tap.permission_check
       ├─ cache.lookup (cache_hit: true/false)
       ├─ tap.select_connection          (on cache miss)
       ├─ zakofish.audio_request         (on cache miss)
       └─ cache.write                    (background, on cache miss)
  └─ tap.disconnect (uptime_secs)
```

### HTTP request (`hq-boot`)

`TraceLayer` from `tower-http` automatically creates a span for every HTTP request handled by the Axum backend. The `AuthUser` extractor records `user_id` on the active span once the JWT is validated.

### HQ service methods

Key methods in `TapService` and `AuthService` are instrumented with `#[tracing::instrument]`:

| Span | Fields |
|---|---|
| `tap::create` | `user_id` |
| `tap::get_tap_with_access` | `tap_id` |
| `tap::update_tap` | `tap_id`, `user_id` |
| `tap::delete_tap` | `tap_id`, `user_id` |
| `tap::get_tap_internal` | `tap_id` |
| `auth.authenticate` | — |
| `auth.get_user` | `user_id` |
| `auth.refresh_token` | `user_id` |

---

## What metrics are emitted

### HQ (`meter: "hq"`)

| Metric | Type | Labels | Notes |
|---|---|---|---|
| `hq_http_requests_total` | Counter | `method`, `path`, `status` | HTTP layer (`hq-backend`) |
| `hq_http_request_duration_seconds` | Histogram | `method`, `path` | HTTP layer (`hq-backend`) |
| `hq_play_attempts_total` | Counter | `attempt` (1-based), `result`: ok/error | One increment per attempt sent to the audio engine; a message that needed its retry shows `attempt=1,result=error` then `attempt=2,result=ok` |
| `hq_messages_dropped_total` | Counter | `dropped_reason` | One increment per channel a message failed to reach, after at most one retry. See below for the reason vocabulary |

Both play counters are created lazily on the first increment, so a fresh pod
shows neither family on `/metrics` until a message has actually been played or
dropped — scrape after some traffic, not right after start.

Metric names on `/metrics` are exactly the instrument names below (and the names
OTLP/ClickStack carries): the exporter in `crates/telemetry` is built with the
`_total` and unit suffixes turned off, so a counter is not reported as
`…_total_total` and a `…_seconds` histogram is not reported as `…_seconds_seconds`.

`dropped_reason` is one of:

| Value | Meaning | Counted |
|---|---|---|
| `tap_unavailable` | No tap connection, or the tap is unknown | yes |
| `request_timeout` | The taphub request timed out (both attempts) | yes |
| `permission_denied` | taphub answered that the user may not use this tap | yes |
| `no_consumer` | The engine refused: the session has nothing that would play | yes |
| `engine_error` | An engine-side transport or internal failure | yes |
| `not_in_channel` | Routing placed the message in no channel Zako is in | no — logged at `debug`, ordinary state |
| `mapping_empty` | Text mapping produced nothing to say | no — logged at `debug`, ordinary state |


### TapHub (`meter: "taphub"`)

| Metric | Type | Labels | Notes |
|---|---|---|---|
| `taphub_connected_taps` | UpDownCounter | — | Current live connections |
| `taphub_tap_auth_total` | Counter | `result`: ok/rejected | Auth attempts |
| `taphub_audio_requests_total` | Counter | `tap_id`, `result`: ok/cache_hit/error | |
| `taphub_audio_request_duration_seconds` | Histogram | `tap_id`, `cache_hit` | |
| `taphub_cache_hits_total` | Counter | `tap_id`, `request_type` | |
| `taphub_connection_duration_seconds` | Histogram | `tap_id` | On disconnect |
| `taphub_active_streams` | UpDownCounter | — | Live relay streams |
| `taphub_tap_conn_draining_total` | Counter | `reason` | Connections that announced a drain before leaving (`drain_signal`) |
| `taphub_tap_conn_drain_to_removal_seconds` | Histogram | `tap_id` | Tap's stated drain start → out of the routing table. Measured from the tap's own clock in `TapDraining`, so it is the whole hop, not this process's handling of it |
| `taphub_no_tap_available_total` | Counter | `tap_id`, `held` | A request found no routable connection. `held` separates "every replica is draining" (`held` > 0) from "this tap is not connected" (`held` = 0); the caller sees `TapUnavailable` either way |

### Audio Engine (`meter: "zako3-audio-engine"` via Prometheus registry)

Audio engine metrics are emitted as Prometheus metrics and scraped directly. The
registry setup is the shared one in `crates/telemetry` (`zako3_telemetry::init`);
`services/audio-engine/telemetry/` is an older copy of it that no service builds.

---

## Verification checklist

After starting the stack with `OTLP_ENDPOINT` set:

```sh
# 1. Health checks
curl http://localhost:9091/health   # hq → 200 OK
curl http://localhost:9092/health   # taphub → 200 OK

# 2. Prometheus metrics exposed
curl http://localhost:9091/metrics | grep hq_http
curl http://localhost:9092/metrics | grep taphub_connected_taps

# 3. Traces reaching OpenObserve
# Open http://localhost:5080 → Traces → search by service.name = "hq" or "taphub"
```

If traces are not appearing:
- Confirm `OTLP_ENDPOINT` points to the gRPC port (`5081`).
- Check that OpenObserve is running: `podman ps | grep openobserve`.
- Look for `"Telemetry server listening on"` in the service stdout to confirm the metrics port is up.
