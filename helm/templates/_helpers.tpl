{{/*
Expand the name of the chart.
*/}}
{{- define "zako3.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "zako3.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "zako3.labels" -}}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version | replace "+" "_" }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels for a component
Usage: include "zako3.selectorLabels" (dict "component" "hq" "Release" .Release)
*/}}
{{- define "zako3.selectorLabels" -}}
app.kubernetes.io/name: {{ .component }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Image reference helper
Usage: include "zako3.image" (dict "registry" .Values.image.registry "name" "hq" "tag" .Values.image.tag "pullPolicy" .Values.image.pullPolicy)
*/}}
{{- define "zako3.imageRef" -}}
{{- if .registry -}}
{{ .registry }}/{{ .name }}:{{ .tag }}
{{- else -}}
{{ .name }}:{{ .tag }}
{{- end }}
{{- end }}

{{/*
Secret key ref — resolves to existing secret or chart-created secret.
Usage: include "zako3.secretKeyRef" (dict "secretName" "hq-secret" "existingName" .Values.hq.existingSecret.name "key" "jwt-secret")
*/}}
{{- define "zako3.secretKeyRef" -}}
secretKeyRef:
  name: {{ if .existingName }}{{ .existingName }}{{ else }}{{ .secretName }}{{ end }}
  key: {{ .key }}
{{- end }}

{{/*
nodeAffinity helper — renders affinity.nodeAffinity block.
Local (per-service) value takes precedence over global; both empty = nothing rendered.
Usage: include "zako3.nodeAffinity" (dict "global" .Values.nodeAffinity "local" .Values.hq.nodeAffinity)
*/}}
{{- define "zako3.nodeAffinity" -}}
{{- $aff := coalesce .local .global -}}
{{- with $aff }}
affinity:
  nodeAffinity:
    {{- toYaml . | nindent 4 }}
{{- end }}
{{- end }}

{{/*
PVC name for a component.
existingClaim wins (claim is managed outside the chart), then an explicit name
override, else "<fullname>-<default>".
Usage: include "zako3.pvcName" (dict "root" . "persistence" .Values.cache.persistence "default" "taphub-cache")
*/}}
{{- define "zako3.pvcName" -}}
{{- $p := .persistence | default dict -}}
{{- if $p.existingClaim -}}
{{ $p.existingClaim }}
{{- else if $p.name -}}
{{ $p.name }}
{{- else -}}
{{ include "zako3.fullname" .root }}-{{ .default }}
{{- end -}}
{{- end }}

{{/*
storageClassName line for a PVC — per-service value wins over the global one.
"-" renders an empty class (binds only pre-provisioned volumes); empty renders nothing.
*/}}
{{- define "zako3.storageClassName" -}}
{{- $p := .persistence | default dict -}}
{{- $sc := $p.storageClass | default .root.Values.storageClass -}}
{{- if eq $sc "-" -}}
storageClassName: ""
{{- else if $sc -}}
storageClassName: {{ $sc | quote }}
{{- end -}}
{{- end }}

{{/*
PersistentVolumeClaim for a component. Renders nothing when persistence.existingClaim
is set, since that claim is managed outside the chart.
Usage: include "zako3.pvc" (dict "root" . "persistence" .Values.clickstack.persistence
         "default" "clickstack-data" "size" "100Gi" "accessMode" "ReadWriteOnce")
*/}}
{{- define "zako3.pvc" -}}
{{- $p := .persistence | default dict -}}
{{- if not $p.existingClaim -}}
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: {{ include "zako3.pvcName" . }}
  labels:
    {{- include "zako3.labels" .root | nindent 4 }}
  {{- with $p.annotations }}
  annotations:
    {{- toYaml . | nindent 4 }}
  {{- end }}
spec:
  {{- with include "zako3.storageClassName" . }}
  {{ . }}
  {{- end }}
  accessModes:
    {{- if $p.accessModes }}
    {{- toYaml $p.accessModes | nindent 4 }}
    {{- else }}
    - {{ .accessMode }}
    {{- end }}
  {{- with $p.volumeName }}
  volumeName: {{ . | quote }}
  {{- end }}
  resources:
    requests:
      storage: {{ $p.size | default .size }}
{{- end -}}
{{- end }}

{{/*
Postgres connection URL — in-cluster StatefulSet, or the external URL when
postgres.enabled is false.
*/}}
{{- define "zako3.postgresUrl" -}}
{{- if .Values.postgres.enabled -}}
postgres://{{ .Values.postgres.user }}:{{ .Values.postgres.password }}@{{ include "zako3.fullname" . }}-postgres:5432/{{ .Values.postgres.db }}
{{- else -}}
{{ required "postgres.enabled=false requires either postgres.externalUrl or postgres.existingSecret.name" .Values.postgres.externalUrl }}
{{- end -}}
{{- end }}

{{/*
TimescaleDB connection URL — in-cluster StatefulSet, or the external URL when
timescale.enabled is false.
*/}}
{{- define "zako3.timescaleUrl" -}}
{{- if .Values.timescale.enabled -}}
postgres://{{ .Values.timescale.user }}:{{ .Values.timescale.password }}@{{ include "zako3.fullname" . }}-timescale:5432/{{ .Values.timescale.db }}
{{- else -}}
{{ required "timescale.enabled=false requires either timescale.externalUrl or timescale.existingSecret.name" .Values.timescale.externalUrl }}
{{- end -}}
{{- end }}

{{/*
OTLP endpoint URL

Every service exports to the collector gateway, not to ClickStack directly.
The gateway is the only hop with a retrying exporter queue, so a ClickStack
restart (or a ClickHouse merge that stalls ingest) buffers telemetry instead of
dropping it, and a ClickStack pod that never came up turns into one failing
exporter here rather than eight services each logging a connect error every
second. On 2026-11-17, 8045 of the 8995 HQ log lines in a 24h window were that
connect error.
*/}}
{{- define "zako3.otlpEndpoint" -}}
http://{{ include "zako3.fullname" . }}-otel-collector:4317
{{- end }}

{{/*
OTLP auth header env: ingestion token for the ClickStack hop.

Services no longer carry the token to ClickStack themselves (the gateway does,
in its exporter config); the variables stay so an endpoint pointed back at
ClickStack keeps working without a second change. CLICKSTACK_OTLP_TOKEN must be
defined first so k8s $(VAR) interpolation resolves.
*/}}
{{- define "zako3.otlpAuthEnv" -}}
- name: CLICKSTACK_OTLP_TOKEN
  valueFrom:
    {{- include "zako3.secretKeyRef" (dict
        "secretName" (printf "%s-clickstack-secret" (include "zako3.fullname" .))
        "existingName" .Values.clickstack.existingSecret.name
        "key" .Values.clickstack.existingSecret.otlpTokenKey) | nindent 6 }}
- name: OTEL_EXPORTER_OTLP_HEADERS
  value: "authorization=$(CLICKSTACK_OTLP_TOKEN)"
{{- end }}

{{/*
OTLP + Redis shared env vars
*/}}
{{- define "zako3.sharedEnv" -}}
- name: OTLP_ENDPOINT
  value: {{ include "zako3.otlpEndpoint" . | quote }}
- name: OTEL_FILTER
  value: {{ .Values.telemetry.otelFilter | quote }}
- name: REDIS_URL
  value: "redis://{{ include "zako3.fullname" . }}-redis:6379"
{{- end }}

{{/*
Node log agent config. Kept in a helper so the DaemonSet can hash it into a
pod annotation: a ConfigMap update alone does not restart the pods that read it,
and an agent running yesterday's config is indistinguishable from one running
today's until something breaks.
*/}}
{{- define "zako3.otelAgentConfig" -}}
extensions:
  # Offsets live on the node, not on an emptyDir: an emptyDir only survives a
  # container restart, and a pod delete (node reboot, eviction, a template
  # change) would make every agent re-read the whole of /var/log/pods straight
  # into a 30-day table. With them, `start_at: beginning` means "backfill what
  # the node still has, once".
  file_storage:
    directory: /var/lib/otel-agent/storage
    create_directory: true

receivers:
  file_log/zako3:
    include:
      - /var/log/pods/{{ .Release.Namespace }}_*/*/*.log
    # Not its own logs: while the gateway is down this pod logs an export
    # failure per retry, and feeding that back into the failing pipeline is a
    # loop with nothing to gain. Kubelet names the directory
    # <namespace>_<pod-name>_<uid>, and the pod name carries the release prefix.
    exclude:
      - /var/log/pods/{{ .Release.Namespace }}_{{ include "zako3.fullname" . }}-otel-agent-*/*/*.log
    include_file_path: true
    include_file_name: false
    start_at: beginning
    storage: file_storage
    operators:
      # Parses the CRI header and lifts namespace / pod / container /
      # restart_count out of the file path into resource attributes.
      - type: container

processors:
  batch:
    timeout: 5s
    send_batch_size: 1024

exporters:
  otlp/collector:
    endpoint: "{{ include "zako3.fullname" . }}-otel-collector:4317"
    tls:
      insecure: true
    headers:
      authorization: "${env:CLICKSTACK_OTLP_TOKEN}"
    retry_on_failure:
      enabled: true
      initial_interval: 5s
      max_interval: 30s
      max_elapsed_time: 600s
    sending_queue:
      enabled: true
      queue_size: 512

service:
  extensions: [file_storage]
  pipelines:
    logs:
      receivers: [file_log/zako3]
      processors: [batch]
      exporters: [otlp/collector]
{{- end }}

{{/*
Collector gateway config. Same reason as the agent's: rendered into a hash so a
config-only change rolls the pods instead of waiting for someone to notice.
*/}}
{{- define "zako3.otelCollectorConfig" -}}
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

exporters:
  otlphttp/clickstack:
    endpoint: "http://{{ include "zako3.fullname" . }}-clickstack:4318"
    headers:
      authorization: "${env:CLICKSTACK_OTLP_TOKEN}"
    compression: gzip
    # The queue is half the reason every service exports here instead of to
    # ClickStack directly: a ClickStack restart, or a ClickHouse merge that
    # stalls ingest, costs a delay instead of the telemetry. 512 batches at 5s
    # is 5 to 10 minutes of cushion; max_elapsed_time bounds a longer outage at
    # 10 minutes per batch.
    retry_on_failure:
      enabled: true
      initial_interval: 5s
      max_interval: 30s
      max_elapsed_time: 600s
    sending_queue:
      enabled: true
      queue_size: 512

extensions:
  health_check:
    endpoint: 0.0.0.0:13133

processors:
  # First in every pipeline: the queue is in memory, so without this the exact
  # scenario the queue exists for (ClickStack down for minutes) grows the heap
  # until the OOM killer takes the pod and the buffered telemetry with it.
  # Refusing new data under pressure loses less than dying.
  memory_limiter:
    check_interval: 1s
    limit_percentage: 75
    spike_limit_percentage: 20
  # ClickStack writes into ClickHouse, which dislikes one insert per span.
  batch:
    timeout: 5s
    send_batch_size: 1024

service:
  extensions: [health_check]
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp/clickstack]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp/clickstack]
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp/clickstack]
{{- end }}
