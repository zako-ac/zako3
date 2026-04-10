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
OTLP + Redis shared env vars
*/}}
{{- define "zako3.sharedEnv" -}}
- name: OTLP_ENDPOINT
  value: "http://{{ include "zako3.fullname" . }}-otel-lgtm:4317"
- name: REDIS_URL
  value: "redis://{{ include "zako3.fullname" . }}-redis:6379"
{{- end }}
