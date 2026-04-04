#!/usr/bin/env bash

podman run -d \
  --name zako3-postgres \
  -e POSTGRES_USER=user \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=zako3 \
  -p 5432:5432 \
  postgres:16-alpine
