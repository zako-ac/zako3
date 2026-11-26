# Audio stall instrumentation

What this is for: when a track goes quiet, say **where** it went quiet — the tap,
the path from the tap to taphub, the path from taphub to the engine, or inside the
engine — from one structured log line, without a live packet capture.

The audit that prompted it could only say "the tap was fine, taphub logged the
chunk, and the engine had no frames for fifteen seconds". Everything below exists
to turn that sentence into a named hop.

## The four hops

| hop | recorded by | recorded at |
|---|---|---|
| `tap_to_taphub` | taphub | `services/taphub/core/src/handler/audio_request.rs`, when a chunk is read from the tap (or from the cache) |
| `taphub_to_engine` | taphub | same file, when that chunk is written to the channel the pf3 responder drains |
| `engine_recv` | engine | `crates/taphub-transport/client/src/jitter.rs::pump_chunk`, after the chunk is read off the socket and parsed |
| `buffer_insert` | engine | `crates/opus-jitter/src/lib.rs::accept`, when the jitter buffer actually takes the frame |

Every hop stamps its last frame with `SystemTime` in unix milliseconds, so the
four are comparable on one axis across pods. A negative age in a report means the
two nodes' clocks disagree, which is itself worth knowing.

`taphub_to_engine` is recorded when taphub hands the frame to the transport, not
when the QUIC write completes. The gap between those two is at most the time to
push one chunk onto an `mpsc`, and it is documented here so nobody reads the hop
as a confirmed wire time.

## The two counters that must never be conflated

- `socket_recv_frames` — frames the engine read and parsed off the socket. Counted
  in `pump_chunk`, before anything downstream can drop the frame.
- `buffer_inserted_frames` — frames the jitter buffer accepted (`BufferStats::inserted`).

Interpretation:

- **Both advance, playback stalls** → the break is below the transport (mixer,
  decoder, output), or the buffer is deliberately holding frames back because its
  lead window is full.
- **`socket_recv` advances, `buffer_inserted` does not** → frames reached the
  process and did not reach the buffer. Pump/mpsc starvation, backpressure, or a
  synthetic injection.
- **Both stop** → nothing arrived. The question moves to the upstream hops.

`socket_minus_inserted` in the report is the running difference. A positive value
that grows is the second case; a large value carried over from earlier decimation
is not a stall by itself.

## Gaps and buffer state

Per stream, at the engine:

- `gaps.max_gap_ms` — the longest gap between two frames read off the socket.
- `gaps.over_100ms` / `gaps.over_1000ms` — how many gaps reached those sizes.
  A 20 ms frame cadence means 5 and 50 frames respectively: a handful of
  100 ms gaps is jitter, a 1000 ms gap is a break.

At the stall, from the jitter buffer: `depth`, `capacity`, `max_lead_frames`,
`buffered_ms`, `playhead_ms`, `inserted`, `late`, `dropped`, `concealed`,
`stall_events`.

- `late` — arrived after its slot had already been played past. Rises with
  reordering and with a sender that bursts after a gap.
- `dropped` — discarded at the hard ceiling: a sender that cannot be paced.
- `concealed` — Opus was asked to conceal a missing frame. The audible symptom.
- `depth` at `max_lead_frames` while frames still arrive is backpressure, not
  loss: the buffer is refusing to hoard audio it cannot play.

## The stall line

One JSON object, alone on its line, `target = "zako3::audio::stall"`:

```
WARN zako3::audio::stall: {"event":"audio_stream_stall","ts_unix_ms":...,"stream":"340376482209599488-9f2c...","tap_id":"...","track_id":"...","guild_id":"...","channel_id":"...","request":{...},"stalled_ms":15000,"stream_age_ms":...,"verdict":"taphub_to_engine_path","verdict_reason":"...","fresh_threshold_ms":2000,"hops":{"tap_to_taphub":{"last_frame_unix_ms":...,"age_ms":...,"frames":...,"source":"taphub"},...},"ages":{...},"counters":{"socket_recv_frames":...,"buffer_inserted_frames":...,"socket_minus_inserted":...,"injected_drops":0},"gaps":{...},"buffer":{...},"upstream":{...}}
```

`track_id` comes from the task-local the session layer scopes around the taphub
request (`zako3_audio_probe::with_trace` in `session.rs::play_now`), so no wire
type changed to carry a local logging detail.

## Verdicts and how they are decided

`classify()` in `crates/audio-probe/src/report.rs` is a pure function of the four
hop ages and the buffer state, evaluated in this order:

**Which of these a stall line can actually carry.** A stall line exists only
because the jitter buffer went a full `stall_timeout` without a frame, so at that
instant `engine_recv` is *always* stale and the two "both hops fresh" rows are
not produced by a real stall: a genuinely downstream break (mixer or decoder
stopped consuming) produces no stall line at all, because `yield_pcm` is not
being called and the buffer's timeout never runs. That case is covered by the
track watchdog and `mixer.idle_sources`, not here. The rows stay in the table
because `build_report` is also callable from a context where frames are still
arriving, and it must not mislabel one.

| verdict | condition | means |
|---|---|---|
| `engine_backpressure` | `engine_recv` and `buffer_insert` both fresh, buffer depth at its lead window | the buffer stopped reading on purpose |
| `engine_downstream` | both fresh, depth below the window | the break is below the transport |
| `engine_internal` | `engine_recv` fresh, `buffer_insert` stale | read but never buffered |
| `taphub_to_engine_path` | `taphub_to_engine` fresh, `engine_recv` stale | loss between taphub and the engine |
| `tap_to_taphub_path` | `tap_to_taphub` fresh, `taphub_to_engine` stale | loss between the tap and taphub |
| `tap_or_cache` | nothing fresh | the tap (or the cache reader) is the quiet end |
| `unknown` | socket quiet, no upstream record | the evidence is not enough; the reason says so |

"Fresh" is `age <= fresh_threshold_ms` (default 2000 ms). A frame arrives every
20 ms, so two seconds of quiet is not lateness; it is a stop. Anything fresher is
treated as alive so a jittery clock does not flip a verdict.

## The upstream hop record

taphub publishes its two hops to Redis, key `audio:probe:hop:<stream>`, value a
JSON `UpstreamHops`, TTL 120 s, rewritten at most every 200 ms per stream. The
engine reads it once, at the stall.

- The stream id is derived from the request on both sides
  (`zako3_audio_probe::request_stream_id`: tap id plus the first 16 hex of
  SHA-256 over the discord user id and the request string), so neither side has
  to be told. Two *identical* requests from the same user on the same tap (a
  retry) share one record, which is what you want: they are the same audio. Two
  concurrent plays of the same text by different users do not.
- The write happens off the frame path (`tokio::spawn`) — a slow Redis must never
  make audio late.
- The age rule is the record's own TTL (120 s), deliberately longer than the
  jitter buffer's 15 s stall timeout. A writer that went quiet because the tap
  stopped is exactly the case worth reporting, so its frozen timestamps are
  evidence, not staleness. Only a record that outlived its TTL is refused.
- taphub **deletes** the key when a producer loop ends, so an ended stream cannot
  be read later as a live-but-quiet hop, and the age rule never has to infer it.
- No record at all is reported as `"source":"unavailable"` and produces the
  `unknown` verdict. The device never guesses at a hop it cannot see.
- The engine snapshots its own counters *before* the Redis read, so the report
  describes the instant the buffer gave up rather than the instant a round trip
  later — otherwise a stream that recovered in that window would be blamed on the
  engine.

Cardinality: one key per concurrent stream, dying with its TTL. Metrics carry
only bounded attributes (`verdict`, `stage`); stream and track ids live in the log
line only.

## Synthetic stall injection

A detector that has never fired is not verified. `ZAKO_STALL_INJECT` arms an
injection for an investigation window:

```
ZAKO_STALL_INJECT=<stream-pattern>:<engine-pump|taphub-send>:<seconds>
```

`*` matches every stream, and the rule expires on its own.

- `engine-pump` drops a frame the engine has already read and counted. The socket
  counter climbs, the buffer counter does not: `engine_internal`.
- `taphub-send` drops it at taphub, *after* taphub has recorded the send and
  republished its hop record. The engine's socket goes quiet while taphub's send
  timestamp keeps advancing: `taphub_to_engine_path`. Dropping before that
  bookkeeping would model a different fault (taphub never sent).

`cargo test -p zako3-taphub-transport-client` drives each case through the real
pump, channel and jitter buffer, and asserts both the counter divergence and the
verdict:

- `injection_at_the_pump_diverges_the_counters_and_blames_the_engine` — the
  socket counter climbs, the buffer counter does not, and the verdict is
  `engine_internal` with a fresh `engine_recv` age against a stale
  `buffer_insert` age
- `injection_at_taphub_send_leaves_the_engine_quiet_and_blames_the_path` — the
  socket reads nothing at all while taphub's own record keeps advancing, and the
  verdict is `taphub_to_engine_path`. It drives the real `dropping(Stage::TaphubSend)`
  predicate and a real `UpstreamHopWriter`, so it fails if either half drifts
- `a_tap_that_stops_is_named_as_the_tap`
- `no_upstream_record_is_reported_as_unknown`
- `a_healthy_stream_raises_nothing`

## Metadata retry probe (taphub)

`taphub_meta_retry_total` is the counter for the metadata retry: a connection on
probation (retired at least once for not answering, `strikes > 0`) gets half the
request budget, and when it misses that, the request goes to another connection.
That conjunction is rare enough that the counter sat at zero after it shipped,
and a counter nobody has seen move is not evidence.

`ZAKO_META_STALL_INJECT` on **taphub** makes it happen on demand:

```
ZAKO_META_STALL_INJECT=<tap_id>[:<n>]
```

`n` (default 1) counts from the lowest connection id — arming the variable
restarts the pod and renumbers every connection, so the value is a position and
not an id. For a request routed to one of those connections taphub opens the
chan, holds the request open without sending it, and puts the connection on
probation, so the next request on it takes the half budget and retries:

- `/metrics`: `taphub_meta_attempts_total{attempt="2"}` and
  `taphub_meta_retry_total` climb, `taphub_tap_conn_strikes` shows the probe's
  connection at 1, and `taphub_tap_conn_inflight_requests` keeps the chan it
  never answered.
- logs: `meta stall probe: holding this request open`,
  `tap connection entered probation on the stall probe`, then
  `meta_retry attempt=2: …`.

Why here and not on the tap: stopping a tap *process* kills the transport with
it, so taphub sees a clean close instead of a wedged connection and the retry
path is never entered (2026-11-26 experiment, recorded in the project decision
log). The probe freezes what the counters actually observe — a connected peer
that never answers this request — one hop earlier, and it needs only the taphub
image. It is off unless the variable is set, and it does not survive the next
restart; the window is the variable, not a permanent behaviour.

Two things to keep in mind while a window is open:

- The stalled attempt opens a chan it never answers, on purpose (that is the
  state being modelled), so `taphub_tap_conn_inflight_requests` climbs on the
  probed connection for the length of the window. Do not read a probe window's
  inflight as leak evidence — the signal it was built for is measured outside it.
- `n` has to stay below the number of connections that tap has. With `n` at or
  above it (always the case for a single-replica tap) the retry has nowhere
  healthy to go and just times out twice; the connection also retires after
  three of those, which is an outage rather than a probe.

## Node counters

`scripts/audio-net-probe.sh` samples the kernel's view every 30 s and prints one
JSON line per sample: `nstat` UDP counters and their deltas, UDP socket count and
receive-queue total, and summed `conntrack -S`. It is the other half of "was it
the wire": an application stall with `UdpRcvbufErrors` or a conntrack drop rising
in the same window points at the network rather than at either process. A missing
tool is reported in `errors` rather than silently omitted.

## protofish3 debug

`ZAKO_PROTOFISH3_DEBUG=1` adds `protofish3=debug` to the log filter for that pod.
Off by default: it is verbose enough that leaving it on for normal traffic is its
own problem. Use it only inside an investigation window.

## Limits worth remembering

- The injection at `taphub_send` is not counted as an injected drop at the engine
  (the engine never sees it); the taphub log line records the arming.
- A stream served from the cache has no tap hop. The record's `path` field says
  `cache`, and `tap_to_taphub` there means "the cache reader handed a frame".
- The verdict is only as good as the clocks: NTP skew between the taphub and
  engine nodes shows up as a nonsensical age, not as a wrong verdict, because the
  rules compare ages within one report.
- `stall_events` counts every stall timeout, including ones the buffer recovered
  from. A recovered stall still emits a line: the stutter is real even when it did
  not become an outage.
