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
  storageSize: "20Gi"
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
