# Deployment

## Docker Compose

Copy `.env.example` to `.env` at the repo root and fill in all required values, then:

```sh
docker compose up -d
```

Variables marked **required** have no default and will cause `docker compose` to exit with an error if unset.

---

## Helm

### Prerequisites

- Helm 3.x
- A Kubernetes cluster
- TLS certificate and key for taphub stored in a Secret (see below)

### Quickstart

```sh
helm install zako3 ./helm \
  --set postgres.password=<pg-password> \
  --set timescale.password=<ts-password> \
  --set hq.rpcAdminToken=<token> \
  --set hq.discordClientId=<id> \
  --set hq.discordClientSecret=<secret> \
  --set hq.discordBotToken=<token> \
  --set hq.jwtSecret=<secret> \
  --set taphub.hqRpcAdminToken=<token> \
  --set taphub.tlsSecret=<existing-k8s-secret-name> \
  --set "audioEngines[0].name=bot1" \
  --set "audioEngines[0].discordToken=<token>"
```

Or use a `values.yaml` override file:

```sh
helm install zako3 ./helm -f my-values.yaml
```

### Multiple audio-engine instances

Add one entry per bot to `audioEngines[]`. Each entry creates a separate Deployment.

```yaml
# my-values.yaml
audioEngines:
  - name: "server-a"
    discordToken: "Bot TOKEN_A"
    aeToken: "shared-secret"
  - name: "server-b"
    discordToken: "Bot TOKEN_B"
    aeToken: "shared-secret"
```

### TLS for taphub

taphub requires TLS. Create the Secret before installing the chart:

```sh
kubectl create secret generic taphub-tls \
  --from-file=cert.pem=./cert.pem \
  --from-file=key.pem=./key.pem
```

Then set `taphub.tlsSecret: taphub-tls` in your values. The chart never creates this Secret.

### Using existing Secrets

For each sensitive service, you can reference a pre-existing Secret instead of setting inline values:

```yaml
# Postgres password from an existing Secret
postgres:
  existingSecret:
    name: "my-postgres-secret"
    key: "password"

# All HQ secrets from one existing Secret
hq:
  existingSecret:
    name: "my-hq-secret"
    keys:
      rpcAdminToken: "rpc-admin-token"
      discordClientId: "discord-client-id"
      discordClientSecret: "discord-client-secret"
      discordBotToken: "discord-bot-token"
      jwtSecret: "jwt-secret"

# Per audio-engine instance
audioEngines:
  - name: "bot1"
    existingSecret:
      name: "audio-engine-bot1-secret"
      discordTokenKey: "discord-token"
      aeTokenKey: "ae-token"
```

When `existingSecret.name` is set for a service, the chart skips creating a Secret for it.

### Image registry

All service images are pulled as `<registry>/<name>:<tag>`. Set the registry and tag:

```yaml
image:
  registry: "ghcr.io/minco"
  tag: "1.2.3"
```

### Workers

- **`metrics-sync`** runs as a Deployment. It starts only after Redis is ready (enforced via an initContainer that polls `redis:6379`).
- **`cache-gc`** runs as a CronJob. Default schedule: `*/30 * * * *`. Override with `cacheGc.schedule`.

### Verify

```sh
helm lint ./helm
helm template zako3 ./helm -f my-values.yaml | kubectl apply --dry-run=client -f -
```

---

## Environment Variable Reference

### Infrastructure

#### PostgreSQL (`postgres`)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `POSTGRES_USER` | | `zako3` | Database user |
| `POSTGRES_PASSWORD` | yes | — | Database password |
| `POSTGRES_DB` | | `zako3` | Database name |

#### TimescaleDB (`timescale`)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TIMESCALE_USER` | | `metrics` | Database user |
| `TIMESCALE_PASSWORD` | yes | — | Database password |
| `TIMESCALE_DB` | | `metrics` | Database name |

---

### audio-engine

Source: `services/audio-engine/controller/.env.example`

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AUDIO_DISCORD_TOKEN` | yes | — | Discord bot token |
| `AUDIO_AE_TOKEN` | | `changeme` | Shared auth token between audio-engine and other services |
| `REDIS_URL` | | `redis://redis:6379` | Redis connection (set by compose) |
| `NATS_URL` | | `nats://nats:4222` | NATS connection (set by compose) |
| `PORT` | | `10031` | HTTP listen port |
| `HOST` | | `0.0.0.0` | HTTP bind address |
| `OTLP_ENDPOINT` | | `http://otel-lgtm:4317` | OpenTelemetry collector endpoint (set by compose) |
| `METRICS_PORT` | | `9090` | Prometheus metrics port |

---

### hq

Source: `services/hq/boot/.env.example`

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HQ_RPC_ADMIN_TOKEN` | yes | — | Admin token for the internal gRPC API. Must match `ZK_TH_HQ_RPC_ADMIN_TOKEN` in taphub |
| `HQ_DISCORD_CLIENT_ID` | yes | — | Discord OAuth2 application client ID |
| `HQ_DISCORD_CLIENT_SECRET` | yes | — | Discord OAuth2 application client secret |
| `HQ_DISCORD_REDIRECT_URI` | | `http://localhost:8080/auth/callback` | OAuth2 redirect URI registered in the Discord application |
| `HQ_DISCORD_BOT_TOKEN` | yes | — | Discord bot token |
| `HQ_JWT_SECRET` | yes | — | Secret key used to sign JWTs. Use a long random string |
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` | yes | — | Used to construct `DATABASE_URL` (set by compose) |
| `BACKEND_ADDRESS` | | `0.0.0.0:8080` | HTTP API bind address |
| `RPC_ADDRESS` | | `0.0.0.0:50052` | gRPC bind address |
| `MAPPER_WASM_DIR` | | `/tmp/hq-mappers` | Directory for WASM mapper files |
| `MAPPER_DB_PATH` | | `/tmp/hq-mappers.db` | SQLite path for mapper metadata |
| `OTLP_ENDPOINT` | | `http://otel-lgtm:4317` | OpenTelemetry collector endpoint (set by compose) |
| `METRICS_PORT` | | `9091` | Prometheus metrics port |

---

### taphub

Source: `services/taphub/core/.env.example`

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `ZK_TH_HQ_RPC_ADMIN_TOKEN` | yes | — | Must match `HQ_RPC_ADMIN_TOKEN` |
| `ZK_TH_CERT_FILE` | | `cert.pem` | Path to TLS certificate file inside the container |
| `ZK_TH_KEY_FILE` | | `key.pem` | Path to TLS private key file inside the container |
| `ZK_TH_REDIS_URL` | | `redis://redis:6379` | Redis connection (set by compose) |
| `ZK_TH_HQ_RPC_URL` | | `http://hq:50052` | HQ gRPC URL (set by compose) |
| `ZK_TH_TRANSPORT_BIND_ADDR` | | `0.0.0.0:4000` | Taphub transport bind address |
| `ZK_TH_ZAKOFISH_BIND_ADDR` | | `0.0.0.0:4001` | Zakofish protocol bind address |
| `ZK_TH_CACHE_DIR` | | `/cache` | Directory for audio cache files |
| `ZK_TH_REQUEST_TIMEOUT_MS` | | `13000` | Request timeout in milliseconds |
| `ZK_TH_OTLP_ENDPOINT` | | `http://otel-lgtm:4317` | OpenTelemetry collector endpoint (set by compose) |
| `ZK_TH_METRICS_PORT` | | `9092` | Prometheus metrics port |

---

### cache-gc

Source: `workers/cache-gc/src/config.rs` (CLI args, also readable from env)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CACHE_GC_CACHE_DIR` | yes | `/cache` | Directory containing `cache.db` and `.opus` files |
| `CACHE_GC_MAX_BYTES` | | — | Maximum total cache size in bytes before GDSF eviction runs. Example: `10737418240` (10 GiB) |
| `GC_BATCH_SIZE` | | `50` | Number of entries to evict per GDSF batch |
| `REDIS_URL` | | `redis://redis:6379` | Redis connection for persisting GC metrics (optional; set by compose) |
| `OTLP_ENDPOINT` | | `http://otel-lgtm:4317` | OpenTelemetry collector endpoint (set by compose) |

cache-gc is a one-shot worker. Pair it with a cron job or `docker compose run cache-gc` invocation with a subcommand:

```sh
docker compose run --rm cache-gc run-evict   # evict expired + dangling + GDSF
docker compose run --rm cache-gc run-all     # also validates .opus files
docker compose run --rm cache-gc validate    # probe .opus files only
```

---

### metrics-sync

Source: `workers/metrics-sync/.env.example`

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TIMESCALE_USER` / `TIMESCALE_PASSWORD` / `TIMESCALE_DB` | yes | — | Used to construct `DATABASE_URL` (set by compose) |
| `REDIS_URL` | | `redis://redis:6379` | Redis connection (set by compose) |
| `METRICS_SYNC_INTERVAL_SECONDS` | | `60` | How often to sync metrics from Redis to TimescaleDB |
| `RUST_LOG` | | `info` | Log filter. Example: `info,metrics_sync=debug` |

---

### web

> **Build-time only.** These are Vite `VITE_*` variables baked into the JS bundle at build time. Changing them requires rebuilding the image.

Set via `docker compose build --build-arg` or in the `args:` block of `docker-compose.yml`.

| Variable | Default (compose) | Description |
|----------|-------------------|-------------|
| `VITE_API_BASE_URL` | `/api/v1` | REST API base URL. Relative path works when nginx proxies `/api` to hq |
| `VITE_WS_BASE_URL` | `ws://localhost` | WebSocket base URL |
| `VITE_GRAFANA_URL` | `http://localhost:3000` | Grafana dashboard URL shown in the admin panel |

The GitHub Actions publish workflow uses `wss://zako.ac` and `https://dash.zako.ac` for production builds.
