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

{{/* Bootstrap servers: bundled dev Redpanda, then kafka.brokers, then legacy env.KAFKA_BROKERS. */}}
{{- define "attest.kafkaBrokers" -}}
{{- if .Values.devDependencies.redpanda.enabled -}}
{{- printf "%s-dev-redpanda:9092" (include "attest.fullname" .) -}}
{{- else if .Values.kafka.brokers -}}
{{- .Values.kafka.brokers -}}
{{- else -}}
{{- .Values.env.KAFKA_BROKERS -}}
{{- end -}}
{{- end -}}

{{- define "attest.clickhouseUrl" -}}
{{- if .Values.devDependencies.clickhouse.enabled -}}
{{- printf "http://%s-dev-clickhouse:8123" (include "attest.fullname" .) -}}
{{- else if .Values.clickhouse.url -}}
{{- .Values.clickhouse.url -}}
{{- else -}}
{{- .Values.env.CLICKHOUSE_URL -}}
{{- end -}}
{{- end -}}

{{/* Kafka username/password come from a Secret unless the mechanism is cloud IAM. */}}
{{- define "attest.kafkaUsesSecret" -}}
{{- if and .Values.kafka.existingSecret (ne .Values.kafka.saslMechanism "AWS_MSK_IAM") -}}true{{- end -}}
{{- end -}}

{{/* Ports opened between the release's own pods. */}}
{{- define "attest.internalPorts" -}}
{{- range $name, $svc := dict
  "collector" .Values.services.collector
  "control-plane" .Values.services.controlPlane
  "orchestrator" .Values.services.orchestrator
  "mcp-gateway" .Values.services.mcpGateway
  "workbench" .Values.services.workbench
}}
{{- if and $svc.enabled $svc.port }}
- port: {{ $svc.port }}
  protocol: TCP
{{- end }}
{{- end }}
{{- if .Values.devDependencies.postgres.enabled }}
- port: 5432
  protocol: TCP
{{- end }}
{{- if .Values.devDependencies.clickhouse.enabled }}
- port: 8123
  protocol: TCP
- port: 9000
  protocol: TCP
{{- end }}
{{- if .Values.devDependencies.redpanda.enabled }}
- port: 9092
  protocol: TCP
{{- end }}
{{- end -}}
