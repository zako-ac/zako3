#!/usr/bin/env bash

podman rm -f zako3-timescale 2>/dev/null || true
podman run -d \
  --name zako3-timescale \
  -e POSTGRES_USER=user \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=zako3 \
  -p 5433:5432 \
  timescale/timescaledb:latest-pg17
