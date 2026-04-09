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

Supported per-service keys: `hq`, `taphub`, `metricsSync`, `cacheGc`, and each entry in `audioEngines[]`.

When `nodeAffinity` is empty (`{}`), no `affinity:` block is emitted for that pod.
