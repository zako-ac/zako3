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
         "default" "clickstack-data" "size" "20Gi" "accessMode" "ReadWriteOnce")
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
*/}}
{{- define "zako3.otlpEndpoint" -}}
http://{{ include "zako3.fullname" . }}-clickstack:4317
{{- end }}

{{/*
OTLP auth header env — ingestion token for direct-to-ClickStack export.
CLICKSTACK_OTLP_TOKEN must be defined first so k8s $(VAR) interpolation resolves.
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
