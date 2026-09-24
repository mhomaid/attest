{{- define "attest.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "attest.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s" (include "attest.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "attest.labels" -}}
app.kubernetes.io/name: {{ include "attest.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{- end -}}

{{- define "attest.selectorLabels" -}}
app.kubernetes.io/name: {{ include "attest.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "attest.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "attest.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "attest.objectStoreSecret" -}}
{{- if .Values.objectStore.existingSecret -}}
{{- .Values.objectStore.existingSecret -}}
{{- else -}}
{{- printf "%s-object-store" (include "attest.fullname" .) -}}
{{- end -}}
{{- end -}}
