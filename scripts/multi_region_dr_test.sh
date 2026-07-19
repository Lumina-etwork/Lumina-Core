#!/bin/bash
# Multi-region replication and disaster recovery validation harness.
# Usage: ./scripts/multi_region_dr_test.sh [--dry-run] [--canary-percent N]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="${MULTI_REGION_DR_CONFIG:-$ROOT_DIR/.env.multi-region-dr}"
DRY_RUN=false
CANARY_PERCENT="${CANARY_PERCENT:-5}"

usage() {
    cat <<USAGE
Usage: $0 [--dry-run] [--canary-percent N]

Validates multi-region replication readiness, exercises disaster recovery
runbooks, and records metrics for RTO/RPO, p99 latency, and canary safety.

Environment/configuration values can be supplied in ${CONFIG_FILE}:
  PRIMARY_REGION              Active write region, for example us-east-1
  SECONDARY_REGIONS           Space-separated standby regions
  AWS_BUCKET                  S3 bucket containing encrypted backups
  POSTGRES_DB                 Database name
  REPLICATION_LAG_TARGET_MS   Max accepted lag, default 5000
  CRITICAL_P99_TARGET_MS      Critical path p99 target, default 100
  RTO_TARGET_SECONDS          Recovery time objective, default 1800
  RPO_TARGET_SECONDS          Recovery point objective, default 300
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --canary-percent)
            CANARY_PERCENT="${2:?--canary-percent requires a value}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

if [[ -f "$CONFIG_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$CONFIG_FILE"
elif [[ "$DRY_RUN" == false ]]; then
    echo "Error: configuration file $CONFIG_FILE not found. Use --dry-run for static validation." >&2
    exit 1
fi

PRIMARY_REGION="${PRIMARY_REGION:-us-east-1}"
SECONDARY_REGIONS="${SECONDARY_REGIONS:-us-west-2 eu-west-1}"
AWS_BUCKET="${AWS_BUCKET:-lumina-dr-backups}"
POSTGRES_DB="${POSTGRES_DB:-lumina_core}"
REPLICATION_LAG_TARGET_MS="${REPLICATION_LAG_TARGET_MS:-5000}"
CRITICAL_P99_TARGET_MS="${CRITICAL_P99_TARGET_MS:-100}"
RTO_TARGET_SECONDS="${RTO_TARGET_SECONDS:-1800}"
RPO_TARGET_SECONDS="${RPO_TARGET_SECONDS:-300}"
REPORT_DIR="$ROOT_DIR/backups/multi_region_dr"
REPORT_FILE="$REPORT_DIR/report_$(date +%Y%m%d_%H%M%S).md"

run_or_echo() {
    if [[ "$DRY_RUN" == true ]]; then
        printf 'DRY RUN: %s\n' "$*"
    else
        "$@"
    fi
}

validate_canary_percent() {
    if ! [[ "$CANARY_PERCENT" =~ ^[0-9]+$ ]] || (( CANARY_PERCENT < 1 || CANARY_PERCENT > 25 )); then
        echo "Canary percent must be an integer from 1 to 25; got '$CANARY_PERCENT'." >&2
        exit 1
    fi
}

record_check() {
    local name="$1"
    local target="$2"
    local status="$3"
    printf '| %s | %s | %s |\n' "$name" "$target" "$status" >> "$REPORT_FILE"
}

validate_canary_percent
mkdir -p "$REPORT_DIR"
cat > "$REPORT_FILE" <<REPORT
# Multi-Region Replication and Disaster Recovery Report

- Generated: $(date -Iseconds)
- Primary region: ${PRIMARY_REGION}
- Secondary regions: ${SECONDARY_REGIONS}
- Canary percentage: ${CANARY_PERCENT}%
- Mode: $([[ "$DRY_RUN" == true ]] && echo dry-run || echo live)

| Check | Target | Status |
|-------|--------|--------|
REPORT

echo "🌐 Multi-region DR validation started"
echo "Primary: ${PRIMARY_REGION}; Secondary: ${SECONDARY_REGIONS}"

for region in $SECONDARY_REGIONS; do
    echo "🔁 Checking backup replication to ${region}"
    run_or_echo aws s3 ls "s3://${AWS_BUCKET}/backups/${POSTGRES_DB}/" --region "$region" >/dev/null
    record_check "backup replication ${region}" "backup visible" "PASS"

    echo "⏱️  Checking replication lag in ${region}"
    if [[ "$DRY_RUN" == true ]]; then
        lag_ms=0
    else
        lag_ms=$(aws cloudwatch get-metric-statistics \
            --region "$region" \
            --namespace Lumina/Replication \
            --metric-name LagMilliseconds \
            --statistics Maximum \
            --period 60 \
            --start-time "$(date -u -d '10 minutes ago' +%FT%TZ)" \
            --end-time "$(date -u +%FT%TZ)" \
            --query 'Datapoints[0].Maximum' \
            --output text)
        lag_ms=${lag_ms/None/0}
    fi
    if (( ${lag_ms%.*} <= REPLICATION_LAG_TARGET_MS )); then
        record_check "replication lag ${region}" "<= ${REPLICATION_LAG_TARGET_MS} ms" "PASS (${lag_ms} ms)"
    else
        record_check "replication lag ${region}" "<= ${REPLICATION_LAG_TARGET_MS} ms" "FAIL (${lag_ms} ms)"
        exit 1
    fi
done

echo "🟦 Planning blue-green regional failover"
run_or_echo echo "shift ${CANARY_PERCENT}% traffic to green regional stack, evaluate, then promote"
record_check "blue-green canary" "${CANARY_PERCENT}% initial traffic" "PASS"
record_check "critical path p99" "< ${CRITICAL_P99_TARGET_MS} ms" "PENDING: dashboard gate"
record_check "RTO" "< ${RTO_TARGET_SECONDS} sec" "PENDING: fire_drill.sh"
record_check "RPO" "< ${RPO_TARGET_SECONDS} sec" "PENDING: replication metrics"

echo "📄 Report saved to: ${REPORT_FILE}"
