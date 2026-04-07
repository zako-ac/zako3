#!/usr/bin/env bash

podman rm -f zako3-nats 2>/dev/null || true
podman run -d \
  --name zako3-nats \
  -p 4222:4222 \
  -p 8222:8222 \
  nats:latest -m 8222
