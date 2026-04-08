#!/usr/bin/env bash

podman rm -f zako3-lgtm 2>/dev/null || true

podman run --name zako3-lgtm -p 3008:3000 -p 4317:4317 -p 4318:4318 -d grafana/otel-lgtm
