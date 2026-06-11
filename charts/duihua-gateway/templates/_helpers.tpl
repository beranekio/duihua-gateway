{{- define "duihua-gateway.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "duihua-gateway.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "duihua-gateway.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "duihua-gateway.labels" -}}
helm.sh/chart: {{ include "duihua-gateway.chart" . }}
{{ include "duihua-gateway.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "duihua-gateway.selectorLabels" -}}
app.kubernetes.io/name: {{ include "duihua-gateway.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "duihua-gateway.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "duihua-gateway.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "duihua-gateway.httpPort" -}}
{{- if .Values.http.listenAddr -}}
{{- $parts := splitList ":" .Values.http.listenAddr -}}
{{- last $parts | int -}}
{{- else -}}
{{- .Values.http.port | int -}}
{{- end -}}
{{- end -}}

{{- define "duihua-gateway.bindAddr" -}}
{{- if .Values.http.listenAddr -}}
{{- .Values.http.listenAddr -}}
{{- else -}}
{{- printf "0.0.0.0:%d" (int .Values.http.port) -}}
{{- end -}}
{{- end -}}

{{- define "duihua-gateway.servicePort" -}}
{{- if .Values.service.port -}}
{{- .Values.service.port | int -}}
{{- else -}}
{{- include "duihua-gateway.httpPort" . -}}
{{- end -}}
{{- end -}}

{{- define "duihua-gateway.validate.config" -}}
{{- if .Values.responsesApiStore.enabled -}}
{{- if not .Values.responsesApiStore.endpoint -}}
{{- fail "responsesApiStore.enabled=true requires responsesApiStore.endpoint" -}}
{{- end -}}
{{- end -}}
{{- if .Values.http.listenAddr -}}
{{- $addr := .Values.http.listenAddr -}}
{{- if or (hasPrefix "127.0.0.1:" $addr) (hasPrefix "localhost:" $addr) (eq $addr "127.0.0.1") (eq $addr "localhost") -}}
{{- fail "http.listenAddr must bind to a pod-reachable address (use 0.0.0.0:port, not loopback)" -}}
{{- end -}}
{{- end -}}
{{- end -}}