# zako3 TTS silent loss (2026-11-28)

Why a message typed into a TTS channel sometimes plays nothing, with no drop
anywhere to see. Written after the user's report "자코가 메시지를 읽었다
안읽었다 해" and the follow-up "통방에 들어와있는거 봤는데도 안읽음".

## First: which path is live

The taphub/pf3 material in `docs/investigations/zako-s2b-hypotheses.md` and the
`taphub_tap_*` alerts describe a path that **no longer carries TTS**.
`taps.gateway_v4` for supertonic is `true` (2026-11-26 23:20 decision), so:

```
Discord message -> HQ message_create -> HQ gateway (ws) -> tap synthesizes
   -> HQ gateway backend -> ae-proxy :5000/UDP -> audio-engine sink :5000/UDP
```

Measured 2026-11-28: the two pf3 `supertonic-tap` pods report
`tap_last_request_at_ms = 0` and `tap_chan_open_total = 0` — they serve nothing.
`supertonic-tap-v4-...-btmkr` is the pod doing the work. Every "tap connection
is wedged" theory that ends at taphub is therefore about a path the audio does
not take.

## Three separate losses, measured over 24h (2026-11-27T14:20Z .. 11-28T14:20Z)

| Class | Signal | 24h count | User hears |
|---|---|---|---|
| **A. Relay gap** | tap reports `frames_sent=N`, engine logs `no frame arrived for 15s` | 19 starts, 3 unrecoverable | nothing, or the message ~15 s late |
| **B. No session** | early drop `not_in_channel` with no Zako session in the guild | 15 | nothing; nothing logged above `debug` |
| **C. pf3 metadata timeout** | taphub `request_audio_metadata timed out` | 2 | 5 s delay, then the retry usually lands |

Play attempts in the same window: 799 (`Starting playback`), 788
`Track ended naturally`.

### A is the one that looks like "the connection is wedged"

Request `0f6552d7-feeb-4642-b9e3-efafd330d471` (2026-11-28 10:39Z):

```
tap  -> INFO stream outcome reported ... outcome=Aborted { frames_sent: 1101,
         reason: "timed out waiting for the receiver to confirm the tail" }
HQ   -> WARN stream aborted request_id=0f6552d7... frames_sent=0
         reason=UnrecoverableGap time_to_first_sample_ms=Some(3187)
engine -> ERROR jitter buffer error e=no frame arrived for 15s
engine -> INFO cache copy abandoned reason=UnrecoverableGap request_id=0f6552d7...
```

The tap synthesized and streamed the whole message. The engine received none of
it. The hop between them is **ae-proxy**, and until this change it had:

* no PodMonitor — `aeproxy_datagrams_dropped_total{reason}` and
  `aeproxy_route_lookups_total{result}` existed but were never scraped;
* no log line in ClickStack (`ServiceName='ae-proxy'` has no rows) and none on
  stdout, so a lost datagram left no trace on either plane.

The relay resolves each `request_id` with one Redis read, spawned off the
datagram path. That read had **no timeout**: if it never returned, the id stayed
in `Pending`, the first eight datagrams were held and every one after them was
dropped, and the sender still reported a healthy stream. That is exactly the
shape above.

### B is the "Zako was in the room" case

`resolve_tts_channels` returns an empty list when `get_sessions_in_guild` has no
session, and `message_create` turns that into an early drop. Both "the user is
not in a voice channel" and "the guild has no session at all" became the same
`not_in_channel`, which is logged at `debug` and deliberately kept out of
`hq_messages_dropped_total`.

2026-11-28 13:49:57Z -> 13:51:21Z is a clean example: a `/leave` at 13:49:57 and
a `/join` 84 s later, and the eight messages typed in between were dropped
silently. `get_sessions_in_guild` reads the master bot's serenity cache, so it
also misses a worker bot whose engine has dropped out of HQ's registry — the bot
is visible in the channel while HQ believes no session exists.

## What changed

1. `workers/ae-proxy`: the route lookup runs inside `ZK_AEP_LOOKUP_TIMEOUT_MS`
   (default 2000). A lookup that times out is counted
   (`aeproxy_route_lookups_total{result="timeout"}`), logged with the request id,
   and resolved as "no route" so the id stops being held and starts being
   dropped with a reason. Test: `a_lookup_that_never_answers_is_given_up_on_and_counted`.
2. `hq-core` / `hq`: a new early-drop reason `no_bot_session` for "the guild had
   no session", logged at `info` with the author's voice channel and reading
   rule, and counted in `hq_messages_unreadable_total{reason}` — a counter that
   only covers messages nobody could have heard, so it does not drown in
   `mapping_empty`.
3. `c1-flux`: a PodMonitor for ae-proxy and two rules,
   `ZakoAudioRelayLookupTimeout` and `ZakoTtsNoBotSession`.

## What this does not settle

* Whether the 19 relay gaps are all the unbounded lookup. The bound turns a hung
  read into a counted drop; the PodMonitor is what will say whether it happens.
* Why a read would hang at all. zako3-redis is a single replica and the same
  process serves tap routing, sessions and the sink registry.
* The tap's payload goes through a **public LoadBalancer** (`zako3-ae-proxy-ingest`,
  198.135.100.42:5000) from pods in the same cluster, so the datagrams hairpin.
  That is a second candidate for packet loss and has not been measured.
* B's repair (joining on demand, or re-establishing a session when the engine is
  connected but its state is gone) is not in this change: it changes what Zako
  does with a channel nobody asked it to sit in, and that is a product decision.
