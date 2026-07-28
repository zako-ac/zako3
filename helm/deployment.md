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
    size: "20Gi"
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
`clickhouse/clickstack-all-in-one` image). All services send OTLP directly to ClickStack over
OTLP gRPC (`OTLP_ENDPOINT` → `-clickstack:4317`), attaching the ingestion bearer token via
`OTEL_EXPORTER_OTLP_HEADERS` (`authorization=<token>`). The `-otel-collector` gateway is
retained in the chart but no longer on the hot path.

```yaml
clickstack:
  replicas: 1
  otlpAuthToken: "<ingestion token>"   # OTLP_AUTH_TOKEN — collector authenticates with this
  apiKey: "<hyperdx api key>"          # HYPERDX_API_KEY
  persistence:
    size: "20Gi"
  # or reference a pre-existing Secret:
  existingSecret:
    name: ""
    otlpTokenKey: "otlp-auth-token"
    apiKeyKey: "hyperdx-api-key"
```

Access the HyperDX UI via port-forward:

```bash
kubectl port-forward svc/<release>-clickstack 8080:8080
```

**OpenObserve** is retained in the chart but turned off by default (`openobserve.replicas: 0`).
Its StatefulSet, PVC, and Service still render, so no pod runs until the replica count is raised.
