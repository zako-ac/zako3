# pod-probe — capture a wedged tap/taphub without deleting anything

Two scripts and a Dockerfile. The image is built in CI (`.github/workflows/pod-probe.yml`)
because `apt-get` inside a zako3 pod hangs at connect: a capture image that
installs gdb at run time never finishes installing it.

- `capture.sh` — runs on the target's node with `hostPID`, finds the pod's process
  by its uid in `/proc/<pid>/cgroup`, and prints `/proc/<pid>/task/*/{syscall,stack,wchan,stat}`,
  `/proc/<pid>/net/udp`, `ss -uapn`, the pod's own `/health` and `/metrics`, and
  `gdb -p <pid> -batch -ex 'thread apply all bt'`. Every line is prefixed
  `zako_probe`, so one ClickStack query finds the whole capture. The budget is 30s
  and each section checks the clock before it starts.
- `sweep.sh` — the trigger, run by the `zako3-pod-probe-sweep` CronJob. Asks
  Prometheus two questions (a silent-and-struck hub connection; a tap that took a
  request and did not reply for 60s) and creates a node-pinned capture Job for
  each pod that answers yes. The Job name carries a 10-minute bucket, which is
  the whole cooldown: a second sweep in the same bucket gets 409.
- `force-capture.sh` — capture a named tap now, on demand.

**Deployed from** `c1-flux` `apps/c1/zako3/capture/probe.yml` (ServiceAccount,
Role, RoleBinding, CronJob). The image tag in that file is bumped by hand when
this directory changes, so what runs in production is always a script you can
read at that commit.

**The runbook** — metric names, retention evidence, queries, and how to read a
capture — is the ZAKO3 project's `docs/instrumentation/zako-audio-probes.md`.
