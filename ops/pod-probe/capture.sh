#!/usr/bin/env bash
# zako3 pod-probe: capture one wedged container's state from the node it runs
# on, without deleting, restarting or exec-ing into anything.
#
# The problem this exists for: every S2b incident so far lost its evidence.
# The tap is killed or the pod is deleted before anyone looks, the image has no
# gdb, and /proc alone cannot separate "idle" from "wedged". A capture that
# lands in ClickStack while the pod is still alive is the only version of this
# that survives.
#
# How it gets to a process it does not own: the caller (sweep.sh) pins this pod
# to the target's node and gives it hostPID, so every process on that node is
# visible in /proc. The target is found by its pod uid in /proc/<pid>/cgroup,
# the same lookup the 2026-11-27 SIGSTOP experiment used by hand.
#
# Budget: 30 seconds (CAPTURE_BUDGET_SECS). Each section checks the clock first
# and each external command has its own timeout, so a wedged process that makes
# gdb hang cannot push the capture past the deadline.
#
# Output: every line is prefixed `zako_probe ` so the whole capture is one
# `Body LIKE 'zako_probe%'` query in ClickStack. `meta` lines are key=value,
# `raw` lines are the untouched content of a file or command.
set -uo pipefail

BUDGET_SECS=${CAPTURE_BUDGET_SECS:-30}
START=$SECONDS

SECTIONS_OK=0
SECTIONS_FAILED=0
GDB_RESULT="not-run"

emit() { printf 'zako_probe %s\n' "$*"; }
raw() { sed -e 's/^/zako_probe raw /'; }
left() { echo $(( BUDGET_SECS - (SECONDS - START) )); }

# section <name> <command...> — header, then the command's stdout+stderr as raw
# lines, or a `failed` meta line naming the exit status.
section() {
  local name=$1; shift
  local remaining; remaining=$(left)
  if [ "$remaining" -le 0 ]; then
    emit "skip section=${name} reason=budget_exhausted"
    SECTIONS_FAILED=$((SECTIONS_FAILED + 1))
    return
  fi
  emit "section ${name}"
  local out
  out=$(timeout "$remaining" "$@" 2>&1)
  local rc=$?
  if [ -n "$out" ]; then printf '%s\n' "$out" | raw; fi
  if [ "$rc" -eq 0 ]; then
    SECTIONS_OK=$((SECTIONS_OK + 1))
  else
    emit "failed section=${name} exit=${rc}"
    SECTIONS_FAILED=$((SECTIONS_FAILED + 1))
  fi
}

# find_host_pid <pod-uid> [container-name] — the host PID of the pod's process.
# cgroup v2 puts every process of a pod under .../pod<uid>/<container-id>, and
# the pause container is skipped because it is the sandbox, not the workload.
find_host_pid() {
  local uid=$1 container=${2:-} pid comm cg
  for d in /proc/[0-9]*; do
    pid=${d#/proc/}
    [ -r "$d/cgroup" ] || continue
    grep -q "pod${uid}" "$d/cgroup" 2>/dev/null || continue
    if [ -n "$container" ] && ! grep -q "/${container}/" "$d/cgroup" 2>/dev/null; then
      continue
    fi
    comm=$(cat "$d/comm" 2>/dev/null) || continue
    [ "$comm" = "pause" ] && continue
    printf '%s %s\n' "$pid" "$comm"
  done | sort -n | head -1
}

capture_pod() {
  local ns=${TARGET_NAMESPACE:-zako3}
  local pod=${TARGET_POD:?TARGET_POD is required}
  local uid=${TARGET_POD_UID:?TARGET_POD_UID is required}
  local tap=${TAP_ID:-unknown}
  local conn=${CONNECTION_ID:-unknown}
  local pod_ip=${TARGET_POD_IP:-}

  emit "meta capture_begin ts=$(date -u +%Y-%m-%dT%H:%M:%SZ) namespace=${ns} pod=${pod} node=${TARGET_NODE:-$(hostname)} tap_id=${tap} connection_id=${conn} pod_uid=${uid} budget_secs=${BUDGET_SECS}"

  local found pid comm
  found=$(find_host_pid "$uid")
  pid=${found%% *}
  comm=${found#* }
  if [ -z "${found:-}" ] || [ -z "$pid" ]; then
    emit "meta error=target_pod_not_on_this_node pod=${pod} node=${TARGET_NODE:-$(hostname)}"
    emit "meta capture_end result=no_target elapsed_s=$((SECONDS - START))"
    return 0
  fi
  emit "meta host_pid=${pid} comm=${comm}"

  # ---- kernel-side view -----------------------------------------------------
  section "proc_status" cat "/proc/${pid}/status"
  section "proc_stat" cat "/proc/${pid}/stat"
  section "proc_wchan" cat "/proc/${pid}/wchan"
  section "proc_limits" cat "/proc/${pid}/limits"
  section "proc_io" cat "/proc/${pid}/io"
  section "proc_ns" ls -l "/proc/${pid}/ns"
  section "threads_ps" ps -Leo pid,tid,stat,wchan:32,comm -p "$pid"

  # Per-thread: what the thread is blocked in (kernel), its syscall arguments,
  # and its kernel stack. /proc/<pid>/task/*/syscall is the cheapest way to tell
  # "epoll_wait forever" (healthy idle) from "stuck inside a read/write".
  local t count=0
  for t in /proc/"${pid}"/task/*; do
    count=$((count + 1))
    if [ "$count" -gt 96 ]; then
      emit "meta threads_truncated_at=96"
      break
    fi
    local tid=${t##*/}
    section "task_${tid}_comm" cat "$t/comm"
    section "task_${tid}_wchan" cat "$t/wchan"
    section "task_${tid}_syscall" cat "$t/syscall"
    section "task_${tid}_stat" cat "$t/stat"
    section "task_${tid}_stack" cat "$t/stack"
  done
  emit "meta thread_dirs=${count}"

  # The socket table. /proc/<pid>/net/udp is the network namespace the target
  # actually lives in (reading /proc/net/udp from this container would give the
  # host's, which is the same file only because we are in the host netns too --
  # say which one is which instead of assuming).
  section "target_net_udp" cat "/proc/${pid}/net/udp"
  section "host_net_udp" cat /proc/net/udp
  section "ss_udp" ss -uapn
  section "ss_udp_info" ss -uapni

  # The pod's own view of itself, over its IP: a tap that answers /health with
  # `in_flight=N silent_ms=M` is wedged in the handler, not in the transport.
  if [ -n "$pod_ip" ]; then
    section "pod_health" curl -m 3 -sS "http://${pod_ip}:8080/health"
    section "pod_metrics" curl -m 3 -sS "http://${pod_ip}:8080/metrics"
  else
    emit "skip section=pod_health reason=no_target_pod_ip"
  fi

  # ---- user-space view ------------------------------------------------------
  # Last, because it is the slowest and the only section that can hang. A
  # 12-second timeout bounds it; when it gives up, the kernel-side view above
  # still says where the process is parked.
  local remaining; remaining=$(left)
  if [ "$remaining" -gt 8 ]; then
    emit "section gdb_threads_and_backtrace"
    local out rc
    out=$(timeout 12 gdb -q -nx -batch \
            -ex "set pagination off" \
            -ex "set height 0" \
            -ex "set width 0" \
            -ex "set sysroot /proc/${pid}/root" \
            -ex "attach ${pid}" \
            -ex "info threads" \
            -ex "thread apply all bt 15" \
            -ex "detach" 2>&1)
    rc=$?
    if [ -n "$out" ]; then printf '%s\n' "$out" | raw; fi
    if [ "$rc" -eq 0 ]; then
      GDB_RESULT=ok
      SECTIONS_OK=$((SECTIONS_OK + 1))
    elif [ "$rc" -eq 124 ]; then
      GDB_RESULT=timeout
      SECTIONS_FAILED=$((SECTIONS_FAILED + 1))
      emit "failed section=gdb_threads_and_backtrace exit=timeout(12s)"
    else
      GDB_RESULT="error:${rc}"
      SECTIONS_FAILED=$((SECTIONS_FAILED + 1))
      emit "failed section=gdb_threads_and_backtrace exit=${rc}"
    fi
  else
    GDB_RESULT=skipped_no_budget
    emit "skip section=gdb_threads_and_backtrace reason=budget_exhausted"
  fi

  emit "meta capture_end result=ok elapsed_s=$((SECONDS - START)) sections_ok=${SECTIONS_OK} sections_failed=${SECTIONS_FAILED} gdb=${GDB_RESULT}"
}

mode=${1:-pod}
case "$mode" in
  pod) capture_pod ;;
  *)
    emit "meta error=unknown_mode mode=${mode}"
    exit 2
    ;;
esac
