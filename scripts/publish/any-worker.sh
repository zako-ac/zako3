#!/usr/bin/env bash
set -euo pipefail

BINARY="$1"
WORKER="$2"
IMAGE="$3"

mkdir -p dist
cp "target/release/$BINARY" dist/
patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 "dist/$BINARY"
podman build -f "workers/$WORKER/Dockerfile" -t "ghcr.io/zako-ac/$IMAGE:latest" .
podman push "ghcr.io/zako-ac/$IMAGE:latest"
