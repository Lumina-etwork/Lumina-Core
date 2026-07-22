#!/usr/bin/env bash
# Pushes pre-commit execution metrics to Prometheus pushgateway.
# Called fire-and-forget from the main dispatcher; never blocks the commit.
set -euo pipefail

PASS="${1:-0}"
FAIL="${2:-0}"
DURATION_MS="${3:-0}"
PUSHGATEWAY="${PROMETHEUS_PUSHGATEWAY_URL:-http://localhost:9091}"
JOB="lumina_pre_commit_hooks"
INSTANCE="${USER:-unknown}@$(hostname -s 2>/dev/null || echo unknown)"
TIMESTAMP=$(date +%s)
STATUS=$([ "$FAIL" -eq 0 ] && echo 0 || echo 1)

PAYLOAD=$(cat <<METRICS
# HELP pre_commit_hook_duration_ms Duration of the full pre-commit hook run in milliseconds
# TYPE pre_commit_hook_duration_ms gauge
pre_commit_hook_duration_ms{instance="${INSTANCE}"} ${DURATION_MS} ${TIMESTAMP}000
# HELP pre_commit_hook_checks_total Number of checks in this run by result
# TYPE pre_commit_hook_checks_total counter
pre_commit_hook_checks_total{instance="${INSTANCE}",result="pass"} ${PASS} ${TIMESTAMP}000
pre_commit_hook_checks_total{instance="${INSTANCE}",result="fail"} ${FAIL} ${TIMESTAMP}000
# HELP pre_commit_hook_exit_status Exit status of the pre-commit hook (0=pass, 1=fail)
# TYPE pre_commit_hook_exit_status gauge
pre_commit_hook_exit_status{instance="${INSTANCE}"} ${STATUS} ${TIMESTAMP}000
METRICS
)

curl -sf --max-time 2 \
  --data-binary "$PAYLOAD" \
  "${PUSHGATEWAY}/metrics/job/${JOB}/instance/${INSTANCE}" \
  2>/dev/null || true
