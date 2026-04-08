#!/usr/bin/env bash

podman rm -f zako3-openobserve 2>/dev/null || true
podman run -d \
  --name zako3-openobserve \
  -p 5080:5080 \
  -p 5081:5081 \
  -e ZO_ROOT_USER_EMAIL=admin@mincomk.com \
  -e ZO_ROOT_USER_PASSWORD=admin123 \
  openobserve/openobserve
