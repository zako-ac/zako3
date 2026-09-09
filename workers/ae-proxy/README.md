# ae-proxy

A request-id-keyed UDP relay that holds no key material.

While a single public IP is shared, HQ tells taps to send their audio here, and
this forwards each datagram to the sink that owns its `request_id`. Once every
sink has a public IP of its own, HQ puts the sink's address in `deliver_to` and
this worker is deleted — a pure removal, with no change to the wire format.

## The name is narrower than the job

Preloads route to the **cache worker** through here too, and the proxy cannot
tell a cache worker from an audio engine. That is deliberate: it is what keeps
the state a plain `request_id → SocketAddr` map and lets the preload path reuse
the relay for free. `udp-proxy` would be the honest name; `ae-proxy` is the one
it was given.

## How it routes

It reads only the 22-byte cleartext protofish4 header, and only the
`request_id` out of it, via `Header::peek_request_id` — which tolerates packet
kinds it does not recognise, so new kinds ship on the endpoints without
redeploying this. It never decrypts, and never parses a payload.

Both directions run on **one socket**. A tap's NAT keys its mapping on
`(tap_source, proxy_destination)`, so replying from a per-sink ephemeral socket
would mean a symmetric NAT silently drops every `Ack` and every `Nack`. Sides
are told apart by source address instead:

- The **tap** is learned from the first datagram for a request id and pinned.
  Without that, anyone who guessed a request id could rebind the mapping and
  redirect all of a transfer's control traffic.
- The **sink's reply address** is learned the same way, from the first datagram
  that is not the tap. It is *not* compared against the address the route
  resolved to: that is a service address, and a reply comes from the pod that
  sent it. Requiring equality there would drop every ack — in-cluster only, and
  silently.

A third party that both guesses a 128-bit request id and beats the real sink to
it costs that one transfer its NACK recovery, which degrades to no caching. It
cannot redirect anything, because both ends pin on first use.

## Bounds

Every piece of state is capped, and each cap is separate on purpose.

| Setting | Default | Bounds |
| --- | --- | --- |
| `ZK_AEP_MAX_ROUTES` | 4096 | Concurrent request ids |
| `ZK_AEP_MAX_PENDING` | 512 | Datagrams held across all in-flight lookups |
| `ZK_AEP_MAX_NEGATIVE` | 4096 | Ids remembered as unroutable |
| `ZK_AEP_ROUTE_IDLE_SECONDS` | 300 | Idle before a route is swept |
| `ZK_AEP_NEGATIVE_TTL_SECONDS` | 5 | How long "no route" is believed |

Negative entries get their own budget because the ids in them are
attacker-chosen: sharing one budget would let a spray of guessed ids evict every
live transfer, which is far worse than losing the negative cache.

Eviction is load-bearing rather than a safety net: HQ writes routes with a
one-hour TTL and does not always delete them on completion, so
`aeproxy_routes_evicted_total` rising is the signal that `MAX_ROUTES` is too low.

Route lookups run off the datagram path — inline Redis reads would let a spray
of guessed ids stall the relay. The opening datagrams of a transfer are held
until the lookup returns, because a sender that opens with a burst can empty its
whole send window before the route exists, leaving nothing to trigger the sink's
first `Ack` and killing the transfer outright rather than losing a frame.

## Endpoints

`/healthz` and `/metrics` on `METRICS_PORT`. UDP cannot be probed, so this is
what the kubelet checks; health flips only after the UDP socket is bound.
