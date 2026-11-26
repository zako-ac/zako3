#!/usr/bin/env bash
# zako3 pod-probe: the trigger.
#
# Runs as a CronJob every minute (c1-flux `apps/c1/zako3/capture`). Asks
# Prometheus two questions, and for every pod that answers yes creates a
# node-pinned capture Job that runs capture.sh on the node the pod lives on.
#
# Two triggers, because the two halves of S2b look different from each side:
#
#   1. hub side  `taphub_tap_conn_silent_seconds > 60 and taphub_tap_conn_strikes > 0`
#      The hub stopped getting answers from a connection AND has already evicted
#      it (strikes are what "we asked and got nothing" counts; silence alone is
#      just an idle connection, and after 2026-11-27 every idle connection sits
#      above 60s at all times). The metric carries tap_id, not a pod, so every
#      pod of that tap is captured -- at two replicas we cannot tell which one
#      holds the connection, and guessing wrong costs the capture.
#
#   2. tap side  `(tap_last_request_at_ms - tap_last_reply_at_ms) > 60000`
#      A tap took a request and wrote no reply for a minute. This one carries
#      the pod label, so exactly one capture happens.
#
# Cooldown without any state: the Job name carries a 10-minute bucket, so a
# second sweep inside the same bucket gets 409 AlreadyExists from the API and
# does nothing. There is no ConfigMap to lose, and two sweeps racing cannot
# double-capture.
#
# Nothing here deletes, restarts or execs into anything, and no ephemeral
# container is ever created on the target pod (the API for removing those does
# not exist -- three of them are still on `supertonic-tap-fb7585877-9mtrd` from
# the 2026-11-27 experiments).
set -uo pipefail

PROM_URL=${PROM_URL:-http://kube-prometheus-stack-prometheus.monitoring.svc:9090}
NAMESPACE=${TARGET_NAMESPACE:-zako3}
PROBE_IMAGE=${PROBE_IMAGE:?PROBE_IMAGE is required (the pod-probe image to run)}
SILENT_SECONDS=${SILENT_SECONDS:-60}
COOLDOWN_SECS=600

HUB_QUERY=${HUB_QUERY:-"taphub_tap_conn_silent_seconds > ${SILENT_SECONDS} and taphub_tap_conn_strikes > 0"}
TAP_QUERY=${TAP_QUERY:-"(tap_last_request_at_ms - tap_last_reply_at_ms) > 60000"}

DRY_RUN=0
FORCE_TAP=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --force-tap) FORCE_TAP=${2:?--force-tap needs a tap label value}; shift ;;
    --query) HUB_QUERY=${2:?--query needs a PromQL expression}; shift ;;
    --tap-query) TAP_QUERY=${2:?--tap-query needs a PromQL expression}; shift ;;
    *) echo "zako_probe sweep error=unknown_arg arg=$1"; exit 2 ;;
  esac
  shift
done

emit() { printf 'zako_probe sweep %s\n' "$*"; }
now=$(date +%s)
bucket=$(( now / COOLDOWN_SECS ))

SA=/var/run/secrets/kubernetes.io/serviceaccount
API=https://kubernetes.default.svc
CURL_API=(curl -sS --cacert "${SA}/ca.crt" -H "Authorization: Bearer $(cat "${SA}/token")")

prom_query() {
  curl -sS -G "${PROM_URL}/api/v1/query" --data-urlencode "query=$1" -m 10
}

# create_capture_job <pod> <node> <pod-uid> <tap-id> <connection-id> <pod-ip> <reason>
create_capture_job() {
  local pod=$1 node=$2 uid=$3 tap=$4 conn=$5 ip=$6 reason=$7
  local safe_pod=${pod//[^a-z0-9-]/-}
  local name="cap-${safe_pod}-${bucket}"
  # Job names are capped at 63 characters; a longer one is trimmed from the
  # middle of the pod name rather than losing the bucket (the cooldown).
  if [ "${#name}" -gt 63 ]; then
    name="cap-${safe_pod:0:40}-${bucket}"
  fi

  emit "plan job=${name} pod=${pod} node=${node} tap_id=${tap} connection_id=${conn} reason=${reason}"
  if [ "$DRY_RUN" -eq 1 ]; then
    return 0
  fi

  local body
  body=$(jq -n \
    --arg name "$name" --arg ns "$NAMESPACE" --arg image "$PROBE_IMAGE" \
    --arg pod "$pod" --arg node "$node" --arg uid "$uid" --arg tap "$tap" \
    --arg conn "$conn" --arg ip "$ip" --arg reason "$reason" \
    '{
      apiVersion: "batch/v1",
      kind: "Job",
      metadata: {
        name: $name,
        namespace: $ns,
        labels: {
          "zako.ac/probe": "capture",
          "zako.ac/probe-reason": $reason
        },
        annotations: {
          "zako.ac/target-pod": $pod,
          "zako.ac/target-tap-id": $tap,
          "zako.ac/connection-id": $conn
        }
      },
      spec: {
        backoffLimit: 0,
        activeDeadlineSeconds: 120,
        ttlSecondsAfterFinished: 86400,
        template: {
          metadata: {
            labels: {"zako.ac/probe": "capture"},
            annotations: {
              "zako.ac/target-pod": $pod,
              "zako.ac/target-tap-id": $tap
            }
          },
          spec: {
            restartPolicy: "Never",
            nodeName: $node,
            hostPID: true,
            hostNetwork: false,
            tolerations: [{operator: "Exists"}],
            containers: [{
              name: "capture",
              image: $image,
              imagePullPolicy: "IfNotPresent",
              command: ["/usr/local/bin/capture.sh", "pod"],
              env: [
                {name: "TARGET_NAMESPACE", value: $ns},
                {name: "TARGET_POD", value: $pod},
                {name: "TARGET_POD_UID", value: $uid},
                {name: "TARGET_POD_IP", value: $ip},
                {name: "TARGET_NODE", value: $node},
                {name: "TAP_ID", value: $tap},
                {name: "CONNECTION_ID", value: $conn},
                {name: "CAPTURE_BUDGET_SECS", value: "30"}
              ],
              resources: {
                requests: {cpu: "50m", memory: "64Mi"},
                limits: {cpu: "500m", memory: "512Mi"}
              },
              securityContext: {
                privileged: true,
                runAsUser: 0
              }
            }]
          }
        }
      }
    }')

  local out rc
  out=$("${CURL_API[@]}" -X POST -H 'Content-Type: application/json' \
          --data "$body" -w '\n%{http_code}' \
          "${API}/apis/batch/v1/namespaces/${NAMESPACE}/jobs")
  rc=$(printf '%s' "$out" | tail -1)
  case "$rc" in
    201) emit "created job=${name} pod=${pod}" ;;
    409) emit "skip job=${name} reason=already_created_in_this_bucket" ;;
    *)   emit "error job=${name} http=${rc} body=$(printf '%s' "$out" | head -n -1 | tr '\n' ' ' | cut -c1-400)" ;;
  esac
}

# pod_json <label-selector> -> [{pod, node, uid, ip, tap}] from the API
pods_for_selector() {
  "${CURL_API[@]}" "${API}/api/v1/namespaces/${NAMESPACE}/pods?labelSelector=$1" \
  | jq -c '[.items[] | {
      pod: .metadata.name,
      node: .spec.nodeName,
      uid: .metadata.uid,
      ip: .status.podIP,
      tap: (.metadata.labels["zako.ac/tap"] // "unknown"),
      tap_id: (.metadata.labels["zako.ac/tap-id"] // "")
    }]'
}

# ---- forced capture: used by the manual path and by the drill ----------------
if [ -n "$FORCE_TAP" ]; then
  emit "mode=force tap=${FORCE_TAP} bucket=${bucket}"
  pods=$(pods_for_selector "zako.ac/service=tap,zako.ac/tap=${FORCE_TAP}")
  count=$(printf '%s' "$pods" | jq 'length')
  emit "matching_pods=${count}"
  printf '%s' "$pods" | jq -c '.[]' | while read -r p; do
    create_capture_job \
      "$(printf '%s' "$p" | jq -r .pod)" \
      "$(printf '%s' "$p" | jq -r .node)" \
      "$(printf '%s' "$p" | jq -r .uid)" \
      "$(printf '%s' "$p" | jq -r .tap_id)" \
      "manual" \
      "$(printf '%s' "$p" | jq -r .ip)" \
      "manual"
  done
  exit 0
fi

# ---- trigger 1: hub side -----------------------------------------------------
emit "mode=threshold bucket=${bucket} hub_query=${HUB_QUERY}"
hub=$(prom_query "$HUB_QUERY")
hub_status=$(printf '%s' "$hub" | jq -r '.status // "no_response"')
emit "hub_status=${hub_status}"
if [ "$hub_status" = "success" ]; then
  printf '%s' "$hub" | jq -c '.data.result[] | {
      tap_id: .metric.tap_id,
      connection_id: .metric.connection_id,
      silent: .value[1]
    }' | while read -r r; do
    tap_id=$(printf '%s' "$r" | jq -r .tap_id)
    conn=$(printf '%s' "$r" | jq -r .connection_id)
    silent=$(printf '%s' "$r" | jq -r .silent)
    emit "trigger=hub tap_id=${tap_id} connection_id=${conn} silent_seconds=${silent}"
    pods=$(pods_for_selector "zako.ac/service=tap,zako.ac/tap-id=${tap_id}")
    if [ "$(printf '%s' "$pods" | jq 'length')" -eq 0 ]; then
      emit "warn tap_id=${tap_id} resolves_to_no_pod"
      continue
    fi
    printf '%s' "$pods" | jq -c '.[]' | while read -r p; do
      create_capture_job \
        "$(printf '%s' "$p" | jq -r .pod)" \
        "$(printf '%s' "$p" | jq -r .node)" \
        "$(printf '%s' "$p" | jq -r .uid)" \
        "$tap_id" "$conn" \
        "$(printf '%s' "$p" | jq -r .ip)" \
        "hub-silent"
    done
  done
else
  emit "error hub_query_failed body=$(printf '%s' "$hub" | tr '\n' ' ' | cut -c1-300)"
fi

# ---- trigger 2: tap side -----------------------------------------------------
emit "tap_query=${TAP_QUERY}"
tapres=$(prom_query "$TAP_QUERY")
tap_status=$(printf '%s' "$tapres" | jq -r '.status // "no_response"')
emit "tap_status=${tap_status}"
if [ "$tap_status" = "success" ]; then
  printf '%s' "$tapres" | jq -c '.data.result[] | {
      pod: .metric.pod,
      tap: (.metric["zako.ac/tap"] // "unknown"),
      delta_ms: .value[1]
    }' | while read -r r; do
    pod=$(printf '%s' "$r" | jq -r .pod)
    tap=$(printf '%s' "$r" | jq -r .tap)
    delta=$(printf '%s' "$r" | jq -r .delta_ms)
    emit "trigger=tap pod=${pod} tap=${tap} request_minus_reply_ms=${delta}"
    pods=$(pods_for_selector "zako.ac/service=tap")
    one=$(printf '%s' "$pods" | jq -c --arg p "$pod" '[.[] | select(.pod == $p)][0] // empty')
    if [ -z "$one" ]; then
      emit "warn pod=${pod} not_found_in_namespace"
      continue
    fi
    create_capture_job "$pod" \
      "$(printf '%s' "$one" | jq -r .node)" \
      "$(printf '%s' "$one" | jq -r .uid)" \
      "$(printf '%s' "$one" | jq -r .tap_id)" \
      "unknown" \
      "$(printf '%s' "$one" | jq -r .ip)" \
      "tap-stalled"
  done
else
  emit "error tap_query_failed body=$(printf '%s' "$tapres" | tr '\n' ' ' | cut -c1-300)"
fi

emit "sweep_end elapsed_s=$(( $(date +%s) - now ))"
