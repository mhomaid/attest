{{- define "attest.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "attest.fullname" -}}
{{- printf "%s" (include "attest.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "attest.labels" -}}
app.kubernetes.io/name: {{ include "attest.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "attest.image" -}}
{{- printf "%s/attest-%s:%s" .registry .name .tag -}}
{{- end -}}
