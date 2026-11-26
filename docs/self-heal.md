# Self-heal: silent tap connections, and the tap's own deadline

What this is for: **nobody has to delete a pod to make Zako talk again.**

Four days of S2b incidents all ended the same way - a tap connection stopped
answering, taphub kept it in the routing set or cycled it forever, and recovery
was a human running `kubectl delete`. The 2026-11-27 00:25 measurement is the
whole argument: when taphub was restarted, the wedged connection was closed by
the restart and the tap reconnected and answered again **inside 90 seconds**.
Nothing about the tap needed fixing. The connection did.

This document covers the three mechanisms that now do that automatically, and
exactly which failure each one catches.

## The three halves

| mechanism | lives in | fires when | recovery |
|---|---|---|---|
| **Hub-side close** | taphub (`self_heal_heartbeat`) | a connection has been retired at least once **and** has answered nothing for 300s | taphub closes it; the tap's transport reconnects and re-handshakes |
| **In-process request deadline** | the tap (`zako3-tap-sdk` `watchdog`) | a request is in flight and nothing has been written back for 60s | the process exits 70; kubelet restarts the pod |
| **Liveness probe** | c1-flux, per tap Deployment | `/health` returns 503 six times in a row, 10s apart | kubelet restarts the pod |

The second and third overlap on purpose. See [§3](#3-why-both-the-deadline-and-the-liveness-probe).

## 1. taphub closes the connection instead of routing around it

`services/taphub/core/src/hub/mod.rs`.

Retirement (`mark_unanswered` → `suspect_until`) was already there and is not
enough on its own: it is a **cooldown**, so the connection comes back into the
routing set, swallows the next request, and is retired again - forever. The tap
is never told its connection is worthless, and the escalating cooldown only makes
the cycle slower (60s → 15min), not impossible.

The self-heal sweep runs every 30s and closes a connection when **both** hold:

- `suspect_strikes > 0` - it has been retired at least once since it last
  answered anything, and
- `silent_for(now) >= 300s` - it has answered nothing for five minutes.

```rust
should_self_heal(strikes, silent_for) = strikes > 0 && silent_for >= 300s
```

**Why not silence alone.** Measured 2026-11-27 03:00 UTC: ten connections of the
default tap, every one of them silent for 2050–2075 seconds with `strikes = 0`.
That is a quiet night, not a fault. A rule written on silence alone would have
closed all ten every five minutes all night, and the first thing that teaches
everyone is to ignore the alert.

**Why not strikes alone.** A tap under load retires and recovers within seconds;
closing on a strike would turn a busy tap's worst minute into a restart.

**What the close does, in order:**

1. the registry entry is marked `self_healing` in the same critical section that
   selects it - it leaves the routing set **before** the close is sent, so no
   request can be dispatched into the gap (`routable_states` filters it);
2. `zakofish-taphub`'s `ZakofishHub::close_connection` puts a CONNECTION_CLOSE
   with **code 4100** on the wire (application range, distinct from the `0` a
   draining tap sends);
3. `on_tap_disconnected` records `taphub_tap_conn_closed_total{reason="self_heal"}`
   and logs `tap connection closed … reason=self_heal`;
4. the tap's `ReconnectingClient` sees the close, reconnects, and re-handshakes.
   That is the recovery, and it needs no cooperation from the tap's process.

Draining connections are skipped: the tap is already leaving on its own terms,
and replacing a clean departure with a hub-side close would lose the drain
accounting for no gain.

**The close code travels one way.** 4100 is what the *peer* records: the tap's
client surfaces `ConnectionClosed(4100)` and reconnects. This side cannot read it
back: protofish3's driver reports the terminal code of its own loop, and a clean
local close ends with `Some(0)`. So `reason=self_heal` is decided from the
entry's `self_healing` flag, set before the close is sent, and never from the
close code - a reason derived from the code would record the self-heal as
`clean`.

### The metric that made the alert possible

`taphub_connected_taps` is a single total, so "the default tap has no connection"
was not expressible as a query. The gauge heartbeat now also publishes
`taphub_tap_connected{tap_id}`, **including 0** for a tap it has held a
connection for and no longer does. A missing series and a zero are different
claims, and the alert in S11 needs to tell "all its connections are gone" from
"nothing scraped".

## 2. The tap's own request deadline

`libs/tap-sdk/src/watchdog.rs`, wired in `TapBuilder::run`.

A request that has been in flight with **nothing written back** for
`TAP_REQUEST_DEADLINE_SECS` (default 60) ends the process with **exit code 70**.

- **The deadline is on progress, not on duration.** Silence is measured from
  `last_activity_at_ms`, which the request path stamps on arrival and again on
  every audio frame. A stream that has been running for ten minutes and is still
  writing is making progress and is never killed. A request that was taken off
  the wire and then produced nothing is the S2b signature.
- **Idle is never a breach.** Nothing in flight means nothing to fail; a quiet
  voice channel must not restart every replica.
- **Explicit exit, not abort, not OOM.** `std::process::exit(70)` is the only one
  of the three that leaves a readable verdict: the last log line is the breach
  with `in_flight`, `silent_ms` and the deadline; the container's `Reason` stays
  `Error` (not the lie `OOMKilled`); and there is no core dump nobody will read.
  70 is `EX_SOFTWARE`, distinct from 1 (start-up failure) and 137 (SIGKILL).
- **What survives the exit.** The log line does (ClickStack, 30 days). The
  counter `tap_request_deadline_exceeded_total` is incremented first but only
  survives if a scrape lands in the window between the breach and the exit:
  treat the log as the record and the counter as the dashboard's. The pod's
  `RESTARTS` counter carries the fact forward for anything reading kube-state
  metrics.

`TAP_REQUEST_DEADLINE_SECS=0` turns it off. An unparseable value keeps the
default and logs a warning, because a typo silently disabling the watchdog is
the one outcome nobody would notice.

## 3. Why both the deadline and the liveness probe

They fire on different failures, and neither covers the other's:

| the failure | runtime scheduling? | deadline (60s) | liveness probe |
|---|---|---|---|
| a handler is stuck in an await (S2b) | yes | **fires** | fires later (60s of probe failures) |
| the process is frozen - `SIGSTOP`, deadlock below the runtime | **no** | cannot run | **fires** |
| a request is slow but progressing (long stream) | yes | correctly silent | correct (`/health` reads activity) |

The `SIGSTOP` experiment (2026-11-27 04:38 KST) is the second row: the port stays
open, `/health` never answers, and nothing inside the process runs again - a
watchdog inside it is worth nothing there. The liveness probe is the only
observer left. Conversely the probe needs a probe period to notice, so the
in-process deadline wins every race it is in.

Liveness settings, per tap Deployment in `c1-flux`
(`apps/c1/zako3/taps/*-tap.yml`): `httpGet /health` on the `healthcheck` port,
`periodSeconds: 10`, `failureThreshold: 6`, `timeoutSeconds: 2` - 60 seconds to
restart, which is 3× the `/health` stall threshold (`TAP_HEALTH_STALL_SECS=20`)
and ~80× supertonic's worst measured synthesis (753ms). Readiness is unchanged:
a stalled pod should leave the routing set immediately *and* be replaced.

## 4. How this was verified

Unit and integration tests (no cluster needed):

```bash
cargo test -p zakofish-taphub            # hub-side close API over a real QUIC connection
cargo test -p zako3-taphub-core          # the rule, the sweep, the routing filter
cargo test -p zako3-tap-sdk              # the deadline, and a child process that really exits 70
```

| test | what it proves |
|---|---|
| `zakofish_taphub::close_connection_closes_a_peer_that_stopped_answering` | the hub closes a connection whose peer never answers, and the peer's client sees the close |
| `zako3_taphub_core::hub::tests::silence_alone_never_closes_a_connection` | the idle-night measurement cannot trigger a close |
| `…::the_sweep_closes_the_wedged_connection_and_leaves_the_quiet_one` | only the retired-and-silent connection is taken, and it leaves routing in the same step |
| `…::a_connection_is_taken_once_and_a_draining_one_is_left_alone` | overlapping sweeps are harmless; a drain is respected |
| `zako3_tap_sdk::watchdog::tests::a_frame_that_keeps_moving_is_never_a_breach` | the deadline is on progress, not duration |
| `…::a_stalled_request_ends_the_process_with_a_reason` | the real thing: a child process exits 70 and prints the reason |

Production drills (not yet run - S11):

- **hub close**: `ZAKO_META_STALL_INJECT` raises `strikes` on a chosen tap's
  oldest connection without waiting for three real timeouts. Then watch
  `taphub_tap_conn_closed_total{reason="self_heal"}` +1, the same `tap_id`'s
  `taphub_tap_auth_total` +1 inside 90s, and `taphub_tap_connected{tap_id}` go
  0 → 2. The tap pod's `RESTARTS` must stay where it was: the recovery is the
  *connection*, not the pod.
- **liveness**: `SIGSTOP` a scratch tap from the node's PID namespace (container
  PID 1 discards the signal - see the probes doc §12-1), then watch the pod go
  `0/1` at ~30s and `RESTARTS` +1 with `1/1 Running` at ~90s.
- **deadline**: needs a tap image built from this SDK. Inject a handler stall
  (`ZAKO_STALL_INJECT` exists for the engine and taphub, not for the tap); the
  honest alternative is to wait for the first real one and read the exit code.

## 5. What this does not cover

1. **The four taps that are not supertonic get an inert probe today.** Their
   deployed images (`sha-3dc65f8`) predate `zako3#122/#123`, so their `/health`
   is still the constant `"ok"` - the liveness probe is wired but cannot fail.
   It becomes real when those images are rebuilt (F7 in
   `docs/reviews/zako-audio-review.md`). Supertonic (`sha-7d221b3`) has the
   state-based `/health` and is the only tap where the probe is live now.
2. **The request deadline exists in no image yet.** It lives in
   `zako3-tap-sdk`; the `taps` repository pins the SDK by revision and every tap
   image must be rebuilt from a revision that has it. Until then
   `TAP_REQUEST_DEADLINE_SECS` in `taps-common-config` is an unused variable.
3. **A hub-side close is recovery, not diagnosis.** It removes the evidence along
   with the connection. The gdb capture (`docs/pod-probe`) triggers at
   `silent > 60s AND strikes > 0`, roughly four minutes before the self-heal
   fires at 300s, so the capture window is wide - but a capture that fails for
   any reason leaves nothing to look at, because the connection is gone.
4. **taphub is still one replica.** The self-heal runs in the process that holds
   the connection, which is also the process a taphub restart removes. That
   restart still costs every tap a reconnect (D10; the two-replica design needs a
   routing owner per connection and is out of scope).
5. **protofish4 taps have neither mechanism.** `zakofish4-tap` has no
   `TapMetrics`, no state-based `/health` and no deadline. The M4 migration has
   to carry §2 and §3 over, or the taps that move lose the self-heal they gain
   here (`zako-tap-pf4-migration.md` §4/P6).
