#!/usr/bin/env bash
# Capture one tap right now, on demand — the manual half of the probe.
#
# Creates a Job from the sweep CronJob's own template, with `--force-tap` so the
# thresholds are skipped and every pod of that tap is captured. Useful while
# something is visibly broken and you do not want to wait for the sweep to lose
# the race against a health restart.
#
#   ops/pod-probe/force-capture.sh supertonic
#
# The Job it creates is named cap-force-<epoch>, and its stdout lands in
# ClickStack the same way an automatic capture's does: query `zako_probe` in
# service zako3, or by pod name prefix `cap-`.
set -euo pipefail

tap=${1:?usage: force-capture.sh <tap>   (google|papago|youtube|wedding|supertonic)}
ns=${NAMESPACE:-zako3}
cronjob=${CRONJOB:-zako3-pod-probe-sweep}
name="cap-force-$(date +%s)"

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

kubectl -n "$ns" create job "$name" --from="cronjob/${cronjob}" --dry-run=client -o json >"$tmp"
jq --arg t "$tap" '.spec.template.spec.containers[0].args = ["--force-tap", $t]' "$tmp" \
  | kubectl -n "$ns" create -f -

echo "created ${name}; follow it with:"
echo "  kubectl -n ${ns} logs -f job/${name}"
