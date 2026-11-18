# Helm Deployment

## Image

All services pull from the same registry and tag, configured globally:

```yaml
image:
  registry: ""       # e.g. "ghcr.io/yourorg" — empty means no prefix
  tag: latest
  pullPolicy: IfNotPresent
```

## nodeAffinity

Node affinity rules can be set globally (applied to all pods) or overridden per service. The per-service value takes precedence over the global one; both empty means no constraint.

**Global** (applies to all pods unless overridden):

```yaml
nodeAffinity:
  requiredDuringSchedulingIgnoredDuringExecution:
    nodeSelectorTerms:
      - matchExpressions:
          - key: kubernetes.io/arch
            operator: In
            values: [arm64]
```

**Per-service override** — same structure, nested under the service key:

```yaml
hq:
  nodeAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 1
        preference:
          matchExpressions:
            - key: node-role
              operator: In
              values: [compute]
```

Supported per-service keys: `hq`, `taphub`, `metricsSync`, `cache`, and `audioEngine`.

When `nodeAffinity` is empty (`{}`), no `affinity:` block is emitted for that pod.

## Storage

`storageClass` at the top level is the default for every PVC the chart creates. Each service
that owns a volume — `postgres`, `timescale`, `cache`, `clickstack`, `openobserve` — exposes the
same `persistence` block, and any field set there wins over the global default:

```yaml
storageClass: "standard"      # global default

clickstack:
  persistence:
    name: ""                  # PVC name override (default: <release>-clickstack-data)
    existingClaim: ""         # use a PVC managed outside the chart; the chart creates none
    storageClass: "fast-ssd"  # overrides the global storageClass
    size: "100Gi"
    accessModes: []           # overrides the component default
    annotations: {}
    volumeName: ""            # bind to a specific PV
```

- `storageClass: "-"` (global or per-service) renders `storageClassName: ""`, which disables
  dynamic provisioning and binds only pre-provisioned volumes.
- `existingClaim` makes the chart skip PVC creation entirely and mount that claim instead — use it
  when the volume is provisioned by another chart or by hand.
- Default accessMode is `ReadWriteOnce` everywhere except the cache volume, which is `ReadWriteMany`.
- The cache volume belongs to `cache.persistence`; its default name stays `<release>-taphub-cache`
  so existing volumes keep binding. The legacy size keys (`postgres.storageSize`,
  `taphub.cacheStorageSize`, …) are still honoured as a fallback when `persistence.size` is unset.

## External Postgres / TimescaleDB

Set `enabled: false` to skip the bundled StatefulSet, PVC, and Service and point the services at an
external database. Supply the connection URL one of two ways:

```yaml
postgres:
  enabled: false
  externalUrl: "postgres://user:pass@db.example.com:5432/zako3"
```

or reference a Secret you manage, in which case `externalUrl` is not needed and the chart creates no
Secret at all:

```yaml
postgres:
  enabled: false
  existingSecret:
    name: "pg-credentials"
    databaseUrlKey: "database-url"
```

`timescale` works identically (`timescale.enabled`, `timescale.externalUrl`). Rendering fails with an
explicit message if `enabled: false` is set without either a URL or an existing Secret. `DATABASE_URL`
(HQ, emoji-matcher) and `TIMESCALE_DATABASE_URL` (HQ, metrics-sync) resolve through the same
Secret reference either way, so no service config changes.

## Observability

The observability backend is **ClickStack** (ClickHouse + HyperDX, deployed via the
`clickhouse/clickstack-all-in-one` image). Telemetry reaches it through the `-otel-collector`
gateway:

```
services (OTLP_ENDPOINT → -otel-collector:4317) ─┐
-otel-agent DaemonSet (node logs) ───────────────┼→ -otel-collector → -clickstack:4318
                                                 ┘   (queue + retry)      (ClickHouse)
```

- **Every service exports to `-otel-collector:4317`**, not to ClickStack. The gateway is the only
  hop with a retrying exporter queue, so a ClickStack restart costs a delay instead of the
  telemetry. Exporters pointed straight at ClickStack also mean eight services each logging a
  connect error every second whenever ClickStack is down, which is what buried the logs on
  2026-11-17 (8045 of 8995 HQ lines in a 24h window).
- **ClickStack has startup and readiness probes on 4317.** Its entrypoint retries
  `Waiting for ClickHouse to be ready...` forever if clickhouse-server dies, and without a probe
  the pod reports Ready while listening on nothing. The startup window is 15 minutes because the
  first boot runs the ClickStack schema migration. There is no liveness probe on purpose: a
  restart landing in the middle of a migration or a large merge loses more than it recovers.
- **ClickStack's volume is 100Gi by default.** The previous 20Gi filled on 2026-10-14, ClickHouse
  could not reserve space for a merge, and the whole observability backend stayed dead for six
  weeks. Table TTLs keep the working set small: the `otel_*` tables carry 30 days, and the
  ClickHouse system logs (system.trace_log especially) are trimmed.
- **`-otel-agent` is a DaemonSet that tails `/var/log/pods`** for this namespace and ships it to
  the gateway, so a log is still searchable after kubelet's 10Mi x 5 rotation has dropped it
  (roughly eight hours of these services). Retention is `otel_logs`' 30-day ClickHouse TTL. Set
  `logAgent.enabled: false` to turn it off.

```yaml
clickstack:
  replicas: 1
  otlpAuthToken: "<ingestion token>"   # OTLP_AUTH_TOKEN, the gateway authenticates with this
  apiKey: "<hyperdx api key>"          # HYPERDX_API_KEY
  persistence:
    size: "100Gi"
  # or reference a pre-existing Secret:
  existingSecret:
    name: ""
    otlpTokenKey: "otlp-auth-token"
    apiKeyKey: "hyperdx-api-key"

logAgent:
  enabled: true
  image: otel/opentelemetry-collector-contrib:latest
```

Access the HyperDX UI via port-forward:

```bash
kubectl port-forward pod/<release>-clickstack-0 8080:8080
```

**OpenObserve** is retained in the chart but turned off by default (`openobserve.replicas: 0`).
Its StatefulSet, PVC, and Service still render, so no pod runs until the replica count is raised.

## Audio queue age cap

A user's TTS queue is drained in order, so nothing bounds how long a message
waits for its turn: one stalled track holds everything behind it, and the audit
found backlogs of 15+ messages played one to two minutes after they were said.
Late audio is worse than none, and a queue that keeps growing is what makes the
next message later still.

```yaml
audioEngine:
  queueMaxAgeSecs: "90"   # seconds; "0" disables the cap
```

Before a track is started, the engine compares its enqueue time with the cap
(`Queue max age`). Past it, the track is dropped instead of played:

- **TTS queues only** (`tts_<user>`). Music queues are playlists: a track
  waiting its turn is the intended behaviour, not a stall.
- **90s by default.** The median TTS track is 1.78s, so a healthy queue drains
  in seconds and the cap only fires when something upstream is stuck. 90s is
  roughly fifty tracks deep, far past "late" and short of "an answer to a
  question the user has moved on from".
- **Undated tracks play.** A track with no enqueue time (state written before the
  field existed, or restored from it) is never treated as stale: silently
  dropping audio we cannot date is the worse failure.
- **Resuming restarts the clock.** `resume` re-stamps the track, so a queue
  paused for a few minutes does not come back with everything dropped.
- **Read once at startup.** A change rolls the StatefulSet:
  `kubectl -n <ns> rollout restart statefulset/<release>-audio-engine`. No image
  rebuild is needed.

Watch it with:

```promql
audio_queue_stale_skipped_total{queue_name="tts"}
```

and the engine's `Dropping stale queued track` warning, which carries
`track_id`, `age_secs`, `max_age_secs`, `queue_name`, `tap_id` and
`discord_user_id`. Raise the cap if users report messages that were never read;
lower it if audio still arrives too late to be useful. A skip count that rises
while `audio_track_lifecycle_total{event="start"}` stays flat is a queue
discarding rather than draining, and points at the stall behind it.
