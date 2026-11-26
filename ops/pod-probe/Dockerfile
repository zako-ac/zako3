# zako3 pod-probe — a debug image built in CI, never in the cluster.
#
# Why it is built here rather than assembled at run time: `apt-get` inside a
# pod on this cluster hangs at connect (measured 2026-11-27, and again while
# writing this image: the zako3 namespace has no route to the Debian mirrors).
# So gdb, procps, tcpdump and friends have to be baked in ahead of time and
# published to ghcr, where a node-pinned Job can just pull it.
#
# Contents, and what each one is for:
#   gdb         `thread apply all bt` on the wedged process (the one thing
#               /proc cannot tell you: which user-space frame is parked)
#   procps      ps -Leo for a per-thread stat/wchan table in one pass
#   iproute2    `ss -uapn` — socket queues and peer addresses in the host netns
#   tcpdump     manual use only; the automated capture does not run it, because
#               QUIC payloads are encrypted and /proc/net/udp + ss already give
#               the socket state that matters
#   curl, jq    the pod's own /health and /metrics, and building Job JSON
#   coreutils/bash/timeout  the script itself
FROM debian:bookworm-slim

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      bash ca-certificates coreutils curl gdb iproute2 jq procps tcpdump \
 && rm -rf /var/lib/apt/lists/*

COPY capture.sh sweep.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/capture.sh /usr/local/bin/sweep.sh

# The capture writes to stdout only. Nothing is kept inside the container:
# /var/log/pods on the node is the durable path, via otel-agent -> ClickStack.
ENTRYPOINT ["/usr/local/bin/sweep.sh"]
