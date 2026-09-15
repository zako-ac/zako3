#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

pids=()
bash "$SCRIPT_DIR/publish/audio-engine.sh"  & pids+=($!)
bash "$SCRIPT_DIR/publish/taphub.sh"        & pids+=($!)
bash "$SCRIPT_DIR/publish/hq.sh"            & pids+=($!)

fail=0
for pid in "${pids[@]}"; do
  wait "$pid" || fail=1
done
exit $fail
