#!/usr/bin/env bash

podman rm -f zako3-rabbitmq 2>/dev/null || true
podman run -d \
  --name zako3-rabbitmq \
  -p 5672:5672 \
  -p 15672:15672 \
  rabbitmq:3-management
