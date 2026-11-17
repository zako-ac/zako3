#!/usr/bin/env bash
# Node-level packet-loss counters for the audio path.
#
# The audit could not tell whether frames were lost on the wire or never read.
# The application-side counters answer the second half; this answers the first:
# a UDP segment dropped by the kernel, a receive-buffer overflow, or a conntrack
# entry aged out shows up here and nowhere in the app.
#
# Emits ONE JSON object per sample on stdout, so a log collector can take it as
# one record (the same shape the engine's stall line uses). Every sample carries
# the deltas since the previous sample, not raw totals, because a total that has
# been climbing for weeks says nothing about the last minute.
#
# Usage:
#   audio-net-probe.sh                # sample every 30 s, forever
#   audio-net-probe.sh --interval 10  # sample every 10 s
#   audio-net-probe.sh --once         # one sample, deltas against the stored state
#
# State lives in $STATE (default /tmp/audio-net-probe.state), so the first run
# after a restart reports the totals as its deltas and says `first_sample: true`.
#
# Degrades rather than fails: a missing `nstat`, `ss` or `conntrack` shows up as
# `null` for that block and `errors` explains why, so a constrained node still
# tells you what it could see.

set -uo pipefail

INTERVAL=30
ONCE=0
STATE="${AUDIO_NET_PROBE_STATE:-/tmp/audio-net-probe.state}"

while [ $# -gt 0 ]; do
  case "$1" in
    --interval) INTERVAL="${2:-30}"; shift 2 ;;
    --once) ONCE=1; shift ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

NOW_MS() { date +%s%3N; }

# The counters that mean "the network threw audio away".
#   UdpInErrors        datagrams the kernel could not deliver (incl. rcvbuf overflow)
#   UdpRcvbufErrors    the receive buffer was full: the audio case
#   UdpInDatagrams     the denominator, so a drop rate can be computed
#   IpInHdrErrors      malformed packets on the path
#   IpInAddrErrors     packets for an address this node no longer has (VIP moved)
NSTAT_KEYS="UdpInDatagrams UdpInErrors UdpRcvbufErrors UdpNoPorts IpInHdrErrors IpInAddrErrors"

nstat_block() {
  if ! command -v nstat >/dev/null 2>&1; then
    echo '"nstat": null'
    return
  fi
  # -a keeps zero counters, -z suppresses the header, -n keeps numbers raw.
  nstat -az -n 2>/dev/null | awk -v keys="$NSTAT_KEYS" '
    BEGIN { n = split(keys, k, " ") }
    { for (i = 1; i <= n; i++) if ($1 == k[i]) val[k[i]] = $2 }
    END {
      printf "\"nstat\": {"
      for (i = 1; i <= n; i++) {
        v = (k[i] in val) ? val[k[i]] : "null"
        printf "%s\"%s\": %s", (i > 1 ? ", " : ""), k[i], v
      }
      printf "}"
    }'
}

ss_block() {
  if ! command -v ss >/dev/null 2>&1; then
    echo '"ss_udp_sockets": null'
    return
  fi
  # One line per UDP socket: how many are open, and the receive-queue high-water
  # marks. A queue pinned near its limit is the same overflow as a rcvbuf error,
  # seen one layer up.
  local sockets queued
  sockets=$(ss -u -a -n 2>/dev/null | tail -n +2 | wc -l)
  queued=$(ss -u -a -n 2>/dev/null | tail -n +2 | awk '{ q += $2 } END { print q + 0 }')
  printf '"ss_udp_sockets": %s, "ss_recv_q_total": %s' "${sockets:-0}" "${queued:-0}"
}

conntrack_block() {
  if ! command -v conntrack >/dev/null 2>&1; then
    echo '"conntrack": null'
    return
  fi
  # `conntrack -S` is a per-CPU table of counters. Sum them.
  conntrack -S 2>/dev/null | awk '
    NR == 1 { for (i = 1; i <= NF; i++) col[i] = $i; next }
    { for (i = 1; i <= NF; i++) sum[i] += $i }
    END {
      printf "\"conntrack\": {"
      first = 1
      for (i = 1; i <= length(col); i++) {
        if (col[i] == "cpu") continue
        printf "%s\"%s\": %d", (first ? "" : ", "), col[i], sum[i]
        first = 0
      }
      printf "}"
    }'
}

# Current totals as `key value` lines, for the delta.
totals() {
  nstat -az -n 2>/dev/null | awk -v keys="$NSTAT_KEYS" '
    BEGIN { n = split(keys, k, " ") }
    { for (i = 1; i <= n; i++) if ($1 == k[i]) print k[i], $2 }
  '
}

emit_deltas() {
  local first="false"
  if [ ! -f "$STATE" ]; then
    first="true"
    echo "0 UdpInDatagrams" > "$STATE"
  fi
  totals > /tmp/audio-net-probe.current.$$
  awk -v first="$first" '
    FILENAME == ARGV[1] { prev[$1] = $2; next }
    { cur[$1] = $2 }
    END {
      printf "\"udp_delta\": {"
      n = 0
      for (k in cur) {
        d = (k in prev) ? cur[k] - prev[k] : cur[k]
        if (d < 0) d = cur[k]   # counter reset (interface down / node reboot)
        printf "%s\"%s\": %d", (n ? ", " : ""), k, d
        n++
      }
      printf "}, \"first_sample\": %s", first
    }' "$STATE" /tmp/audio-net-probe.current.$$
  mv -f /tmp/audio-net-probe.current.$$ "$STATE"
}

sample() {
  local ts host errors="[]"
  ts=$(NOW_MS)
  host=$(hostname 2>/dev/null || echo unknown)

  local nstat_part ss_part ct_part delta_part
  nstat_part=$(nstat_block)
  ss_part=$(ss_block)
  ct_part=$(conntrack_block)
  delta_part=$(emit_deltas)

  local missing=""
  command -v nstat >/dev/null 2>&1 || missing="${missing:+$missing, }\"nstat missing\""
  command -v ss >/dev/null 2>&1 || missing="${missing:+$missing, }\"ss missing\""
  command -v conntrack >/dev/null 2>&1 || missing="${missing:+$missing, }\"conntrack missing\""
  [ -n "$missing" ] && errors="[$missing]"

  printf '{"event":"audio_net_probe","ts_unix_ms":%s,"host":"%s",%s,%s,%s,%s,"errors":%s}\n' \
    "$ts" "$host" "$nstat_part" "$ss_part" "$ct_part" "$delta_part" "$errors"
}

if [ "$ONCE" = "1" ]; then
  sample
  exit 0
fi

while :; do
  sample
  sleep "$INTERVAL"
done
