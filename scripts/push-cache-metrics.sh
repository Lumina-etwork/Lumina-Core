#!/usr/bin/env bash
# push-cache-metrics.sh — Lee la salida de docker/build-push-action y emite
# metricas de cache al Prometheus Pushgateway configurado.
#
# Uso: ./scripts/push-cache-metrics.sh <metadata-json> <scope> <stage> <duration-segundos>
#
# Variables de entorno requeridas:
#   PUSHGATEWAY_URL — URL base del Pushgateway (sin trailing slash)
#                     Si no esta definida, el script termina sin error.

set -euo pipefail

METADATA_JSON="${1:-}"
SCOPE="${2:-unknown}"
STAGE="${3:-unknown}"
DURATION_SECONDS="${4:-0}"
PUSHGATEWAY_URL="${PUSHGATEWAY_URL:-}"

if [ -z "$PUSHGATEWAY_URL" ]; then
  echo "PUSHGATEWAY_URL no definida — saltando push de metricas"
  exit 0
fi

# Parsear hits/misses del metadata JSON de BuildKit
CACHE_HIT=0
CACHE_MISS=0

if [ -n "$METADATA_JSON" ]; then
  if command -v python3 &>/dev/null; then
    CACHE_HIT=$(echo "$METADATA_JSON" \
      | python3 -c "import json,sys; d=json.load(sys.stdin); print(int(d.get('buildkit.cache.hit', 0)))" \
      2>/dev/null || echo "0")
    CACHE_MISS=$(echo "$METADATA_JSON" \
      | python3 -c "import json,sys; d=json.load(sys.stdin); print(int(d.get('buildkit.cache.miss', 0)))" \
      2>/dev/null || echo "0")
  fi
fi

NOW=$(date -u +%s)

# Enviar metricas al Pushgateway via protocolo de texto de Prometheus
curl -sf --data-binary @- \
  "${PUSHGATEWAY_URL}/metrics/job/ci-docker-cache/instance/${SCOPE}" <<METRICS
# TYPE ci_docker_cache_hits_total counter
ci_docker_cache_hits_total{scope="${SCOPE}",stage="${STAGE}"} ${CACHE_HIT}
# TYPE ci_docker_cache_misses_total counter
ci_docker_cache_misses_total{scope="${SCOPE}",stage="${STAGE}"} ${CACHE_MISS}
# TYPE ci_docker_build_duration_seconds gauge
ci_docker_build_duration_seconds{scope="${SCOPE}",stage="${STAGE}"} ${DURATION_SECONDS}
# TYPE ci_docker_cache_last_push_timestamp_seconds gauge
ci_docker_cache_last_push_timestamp_seconds{scope="${SCOPE}"} ${NOW}
METRICS

echo "Metricas enviadas — scope=${SCOPE} stage=${STAGE} hits=${CACHE_HIT} misses=${CACHE_MISS} duracion=${DURATION_SECONDS}s"
