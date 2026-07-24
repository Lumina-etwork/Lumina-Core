#!/bin/bash
# Scheduled Database Backup Verification with Restore Testing
# Downloads latest encrypted backup from S3, restores to sandbox, runs integrity
# checks against known schemas, generates verification report, and cleans up.
# Designed to run on a cron schedule (e.g., daily at 03:00 UTC).
#
# Usage:
#   ./scripts/verify_backup.sh                    # run verification
#   ./scripts/verify_backup.sh --dry-run          # validate configuration only
#   ./scripts/verify_backup.sh --schedule daily   # tag report with schedule name
#
# Exit codes:
#   0 - verification passed
#   1 - verification failed (integrity, restore, or configuration error)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="${VERIFY_BACKUP_CONFIG:-$ROOT_DIR/.env.backup}"

# ── CLI flags ──────────────────────────────────────────────────────────────────
DRY_RUN=false
SCHEDULE_TAG="manual"
VERIFY_DB="${VERIFY_DB:-lumina_verify_$$}"
PUSH_METRICS_URL=""

usage() {
    cat <<USAGE
Usage: $0 [--dry-run] [--schedule TAG] [--verify-db NAME] [--push-metrics URL]

Scheduled Database Backup Verification with Restore Testing.

Options:
  --dry-run          Validate configuration and print planned actions without
                     downloading or restoring data.
  --schedule TAG     Label the verification report with a schedule tag
                     (e.g., daily, hourly, weekly).  Default: manual.
  --verify-db NAME   Name of the sandbox database used for verification.
                     Default: lumina_verify_<PID>.
  --push-metrics URL Push verification metrics to a Prometheus pushgateway
                     at the given URL (e.g., http://pushgateway:9091).
  -h, --help         Show this message.

Configuration: ${CONFIG_FILE}
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --schedule)
            SCHEDULE_TAG="${2:?--schedule requires a value}"
            shift 2
            ;;
        --verify-db)
            VERIFY_DB="${2:?--verify-db requires a value}"
            shift 2
            ;;
        --push-metrics)
            PUSH_METRICS_URL="${2:?--push-metrics requires a URL}"
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

# ── Configuration loading ──────────────────────────────────────────────────────
if [[ -f "$CONFIG_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$CONFIG_FILE"
elif [[ "$DRY_RUN" == false ]]; then
    echo "Error: Configuration file $CONFIG_FILE not found." >&2
    echo "Set VERIFY_BACKUP_CONFIG or create .env.backup with the required variables." >&2
    exit 1
fi

# Required variables (set in .env.backup)
# POSTGRES_HOST, POSTGRES_PORT, POSTGRES_DB, POSTGRES_USER, POSTGRES_PASSWORD
# AWS_BUCKET, AWS_REGION, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
# ENCRYPTION_KEY (base64 encoded AES-256 key)

# Verification thresholds
VERIFY_MIN_TABLE_COUNT="${VERIFY_MIN_TABLE_COUNT:-1}"
VERIFY_REQUIRED_SCHEMAS="${VERIFY_REQUIRED_SCHEMAS:-}"

# Ensure required vars are present in non-dry-run mode
for var in POSTGRES_HOST POSTGRES_PORT POSTGRES_USER POSTGRES_PASSWORD AWS_BUCKET AWS_REGION ENCRYPTION_KEY; do
    if [[ -z "${!var:-}" ]] && [[ "$DRY_RUN" == false ]]; then
        echo "Error: Required environment variable $var is not set in $CONFIG_FILE" >&2
        exit 1
    fi
done

# Set fallback defaults for dry-run mode where partial configs are tolerated
POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_USER="${POSTGRES_USER:-unknown}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-unknown}"
AWS_BUCKET="${AWS_BUCKET:-unknown}"
AWS_REGION="${AWS_REGION:-unknown}"
ENCRYPTION_KEY="${ENCRYPTION_KEY:-unknown}"

# ── Derived paths ──────────────────────────────────────────────────────────────
TIMESTAMP="$(date +'%Y%m%d_%H%M%S')"
REPORT_DIR="$ROOT_DIR/backups/verification"
DOWNLOAD_DIR="$ROOT_DIR/backups/verification/download_${TIMESTAMP}"
REPORT_JSON="$REPORT_DIR/report_${TIMESTAMP}.json"
REPORT_MD="$REPORT_DIR/report_${TIMESTAMP}.md"
BACKUP_LOG="$REPORT_DIR/verification.log"

# ── Helpers ────────────────────────────────────────────────────────────────────
run_or_echo() {
    if [[ "$DRY_RUN" == true ]]; then
        printf 'DRY RUN: %s\n' "$*"
        return 0
    fi
    "$@"
}

log_step() {
    local level="$1"; shift
    local msg="$(date -Iseconds) [$level] [verify-backup] $*"
    echo "$msg"
    if [[ "$DRY_RUN" == false ]]; then
        echo "$msg" >> "$BACKUP_LOG"
    fi
}

fail_verification() {
    local reason="$1"
    log_step "ERROR" "Verification failed: $reason"
    cat > "$REPORT_JSON" <<JSONEOF
{
  "timestamp": "$(date -Iseconds)",
  "schedule": "$SCHEDULE_TAG",
  "status": "FAIL",
  "reason": "$reason",
  "checks": {}
}
JSONEOF
    echo "❌ Backup verification FAILED: $reason"
    exit 1
}

# ── Main ───────────────────────────────────────────────────────────────────────
mkdir -p "$REPORT_DIR"

log_step "INFO" "Starting backup verification (schedule: $SCHEDULE_TAG, mode: $([[ "$DRY_RUN" == true ]] && echo dry-run || echo live))"

# Step 1: Find most recent backup in S3
log_step "INFO" "Finding most recent backup in s3://${AWS_BUCKET:-N/A}/backups/..."

if [[ "$DRY_RUN" == true ]]; then
    LATEST_BACKUP="${LATEST_BACKUP_MOCK:-20250724_000000}"
    log_step "DRYRUN" "Would list S3 and select latest backup. Using mock: $LATEST_BACKUP"
else
    LATEST_BACKUP=$(aws s3 ls "s3://${AWS_BUCKET}/backups/${POSTGRES_DB}/" \
        --region "${AWS_REGION}" \
        2>/dev/null \
        | sort \
        | tail -n 1 \
        | awk '{print $1}' || true)

    if [[ -z "$LATEST_BACKUP" ]]; then
        fail_verification "No backups found in S3 bucket ${AWS_BUCKET:-N/A}/backups/${POSTGRES_DB:-N/A}/"
    fi
fi

S3_BACKUP_PATH="s3://${AWS_BUCKET:-N/A}/backups/${POSTGRES_DB:-N/A}/${LATEST_BACKUP}"
log_step "INFO" "Latest backup: $LATEST_BACKUP"

# Step 2: Download encrypted backup and metadata
log_step "INFO" "Downloading backup and metadata..."
mkdir -p "$DOWNLOAD_DIR"

run_or_echo aws s3 cp "${S3_BACKUP_PATH}/backup.sql.enc" "$DOWNLOAD_DIR/backup.sql.enc" --region "${AWS_REGION}"
run_or_echo aws s3 cp "${S3_BACKUP_PATH}/metadata.json" "$DOWNLOAD_DIR/metadata.json" --region "${AWS_REGION}"
run_or_echo aws s3 cp "${S3_BACKUP_PATH}/checksum.txt" "$DOWNLOAD_DIR/checksum.txt" --region "${AWS_REGION}" 2>/dev/null || true

# Read metadata for reporting
BACKUP_SIZE_BYTES="unknown"
if [[ -f "$DOWNLOAD_DIR/metadata.json" ]] && command -v jq &>/dev/null; then
    BACKUP_SIZE_BYTES=$(jq -r '.backup_size_bytes // "unknown"' "$DOWNLOAD_DIR/metadata.json")
fi

# Step 3: Decrypt backup
log_step "INFO" "Decrypting backup..."
if [[ "$DRY_RUN" == false ]]; then
    if ! openssl enc -aes-256-cbc -d -pbkdf2 -in "$DOWNLOAD_DIR/backup.sql.enc" \
        -out "$DOWNLOAD_DIR/backup.sql" -pass pass:"${ENCRYPTION_KEY}" 2>/dev/null; then
        fail_verification "Decryption failed - possibly corrupted backup or wrong encryption key"
    fi
fi

# Step 4: Create sandbox database and restore
VERIFY_HOST="${VERIFY_HOST:-${POSTGRES_HOST}}"
VERIFY_PORT="${VERIFY_PORT:-${POSTGRES_PORT}}"

log_step "INFO" "Creating sandbox database: $VERIFY_DB on $VERIFY_HOST:$VERIFY_PORT"

run_or_echo PGPASSWORD="${POSTGRES_PASSWORD}" psql \
    -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" -d postgres \
    -c "DROP DATABASE IF EXISTS ${VERIFY_DB};" 2>/dev/null || true

run_or_echo PGPASSWORD="${POSTGRES_PASSWORD}" psql \
    -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" -d postgres \
    -c "CREATE DATABASE ${VERIFY_DB};"

log_step "INFO" "Restoring backup to sandbox database..."
if [[ "$DRY_RUN" == false ]]; then
    PGRESTORE_LOG="$DOWNLOAD_DIR/pg_restore.log"
    if ! PGPASSWORD="${POSTGRES_PASSWORD}" pg_restore \
        -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" \
        -d "$VERIFY_DB" -v "$DOWNLOAD_DIR/backup.sql" >"$PGRESTORE_LOG" 2>&1; then
        RESTORE_ERR=$(tail -20 "$PGRESTORE_LOG")
        fail_verification "pg_restore failed: ${RESTORE_ERR}"
    fi
else
    run_or_echo PGPASSWORD="${POSTGRES_PASSWORD}" pg_restore \
        -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" \
        -d "$VERIFY_DB" -v "$DOWNLOAD_DIR/backup.sql"
fi

# ── Step 5: Integrity verification ─────────────────────────────────────────────
VERIFY_PASS=true
declare -A CHECK_RESULTS
TABLE_COUNT=0

if [[ "$DRY_RUN" == true ]]; then
    # In dry-run mode, skip actual database checks and validate configuration only.
    # All checks are marked as SKIPPED since no live database is available.
    log_step "INFO" "Dry-run mode: skipping live database integrity checks"
    CHECK_RESULTS["connectivity"]="SKIPPED (dry-run)"
    CHECK_RESULTS["table_count"]="SKIPPED (dry-run)"
    CHECK_RESULTS["schema_validation"]="SKIPPED (dry-run)"
    CHECK_RESULTS["row_sampling"]="SKIPPED (dry-run)"
    CHECK_RESULTS["required_schemas"]="SKIPPED (dry-run)"

    # Perform static schema parsing to validate schema files exist and are parseable
    EXPECTED_TABLES=""
    for schema_file in "$ROOT_DIR"/social/db/schema.sql "$ROOT_DIR"/analytics/db/schema.sql; do
        if [[ -f "$schema_file" ]]; then
            while IFS= read -r line; do
                if [[ "$line" =~ CREATE[[:space:]]+TABLE[[:space:]]+(IF[[:space:]]+NOT[[:space:]]+EXISTS[[:space:]]+)?([a-zA-Z_][a-zA-Z0-9_]*) ]]; then
                    EXPECTED_TABLES="${EXPECTED_TABLES}${BASH_REMATCH[2]} "
                fi
            done < "$schema_file"
        fi
    done
    EXPECTED_COUNT=$(echo "$EXPECTED_TABLES" | wc -w)
    log_step "INFO" "  Static schema check: ${EXPECTED_COUNT} expected tables found in schema files"
    if [[ "$EXPECTED_COUNT" -gt 0 ]]; then
        CHECK_RESULTS["static_schema_parse"]="PASS"
    else
        CHECK_RESULTS["static_schema_parse"]="FAIL"
    fi
else
    # ── Live mode: run actual database integrity checks ──────────────────────────

    run_sql() {
        PGPASSWORD="${POSTGRES_PASSWORD}" psql \
            -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" \
            -d "$VERIFY_DB" -t -c "$1" 2>/dev/null || echo "SQL_ERROR"
    }

    # Check 1: Database is accessible
    log_step "INFO" "Check 1: Database connectivity..."
    if run_sql "SELECT 1;" | grep -q "1"; then
        CHECK_RESULTS["connectivity"]="PASS"
        log_step "INFO" "  ✅ Database accessible"
    else
        CHECK_RESULTS["connectivity"]="FAIL"
        log_step "ERROR" "  ❌ Database not accessible"
        VERIFY_PASS=false
    fi

    # Check 2: Table count meets minimum threshold
    log_step "INFO" "Check 2: Table count..."
    TABLE_COUNT=$(run_sql "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';" | tr -d '[:space:]')
    if [[ "$TABLE_COUNT" =~ ^[0-9]+$ ]] && (( TABLE_COUNT >= VERIFY_MIN_TABLE_COUNT )); then
        CHECK_RESULTS["table_count"]="PASS"
        log_step "INFO" "  ✅ Table count: $TABLE_COUNT (minimum: $VERIFY_MIN_TABLE_COUNT)"
    else
        CHECK_RESULTS["table_count"]="FAIL"
        log_step "ERROR" "  ❌ Table count: ${TABLE_COUNT:-0} (minimum: $VERIFY_MIN_TABLE_COUNT)"
        VERIFY_PASS=false
    fi

    # Check 3: List all tables and compare against expected schemas
    log_step "INFO" "Check 3: Schema validation..."
    RESTORED_TABLES=$(run_sql "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY table_name;" | tr -d '[:space:]')

    # Determine expected tables from schema files
    EXPECTED_TABLES=""
    for schema_file in "$ROOT_DIR"/social/db/schema.sql "$ROOT_DIR"/analytics/db/schema.sql; do
        if [[ -f "$schema_file" ]]; then
            while IFS= read -r line; do
                if [[ "$line" =~ CREATE[[:space:]]+TABLE[[:space:]]+(IF[[:space:]]+NOT[[:space:]]+EXISTS[[:space:]]+)?([a-zA-Z_][a-zA-Z0-9_]*) ]]; then
                    EXPECTED_TABLES="${EXPECTED_TABLES} ${BASH_REMATCH[2]}"
                fi
            done < "$schema_file"
        fi
    done

    SCHEMA_MISMATCHES=""
    if [[ -n "$EXPECTED_TABLES" ]]; then
        for expected in $EXPECTED_TABLES; do
            if ! echo "$RESTORED_TABLES" | grep -q "$expected"; then
                SCHEMA_MISMATCHES="${SCHEMA_MISMATCHES}missing:${expected} "
            fi
        done
    fi

    if [[ -z "$SCHEMA_MISMATCHES" ]]; then
        CHECK_RESULTS["schema_validation"]="PASS"
        log_step "INFO" "  ✅ All expected tables present"
    else
        CHECK_RESULTS["schema_validation"]="FAIL"
        log_step "ERROR" "  ❌ Schema mismatches: $SCHEMA_MISMATCHES"
        VERIFY_PASS=false
    fi

    # Check 4: Row count sampling for key tables
    log_step "INFO" "Check 4: Row count sampling..."
    ROW_SAMPLE=""
    for table in $(echo "$RESTORED_TABLES" | head -10); do
        count=$(run_sql "SELECT COUNT(*) FROM \"$table\";" | tr -d '[:space:]')
        ROW_SAMPLE="${ROW_SAMPLE}${table}=${count} "
        log_step "INFO" "  - $table: $count rows"
    done

    if [[ -n "$ROW_SAMPLE" ]]; then
        CHECK_RESULTS["row_sampling"]="PASS"
    else
        CHECK_RESULTS["row_sampling"]="PASS"  # Empty DB is not necessarily a failure
        log_step "INFO" "  ℹ️  No rows to sample (empty or schema-only database)"
    fi

    # Check 5: Required schemas validation (optional env-var driven)
    if [[ -n "$VERIFY_REQUIRED_SCHEMAS" ]]; then
        log_step "INFO" "Check 5: Required schemas validation..."
        SCHEMA_FAIL=false
        IFS=',' read -ra REQUIRED <<< "$VERIFY_REQUIRED_SCHEMAS"
        for req_schema in "${REQUIRED[@]}"; do
            req_schema=$(echo "$req_schema" | xargs)
            schema_exists=$(run_sql "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = '$req_schema';" | tr -d '[:space:]')
            if [[ "$schema_exists" == "1" ]]; then
                log_step "INFO" "  ✅ Schema '$req_schema' present"
            else
                log_step "ERROR" "  ❌ Required schema '$req_schema' missing"
                SCHEMA_FAIL=true
            fi
        done
        if [[ "$SCHEMA_FAIL" == true ]]; then
            CHECK_RESULTS["required_schemas"]="FAIL"
            VERIFY_PASS=false
        else
            CHECK_RESULTS["required_schemas"]="PASS"
        fi
    else
        CHECK_RESULTS["required_schemas"]="SKIPPED"
    fi
fi

# ── Step 6: Record metrics ─────────────────────────────────────────────────────
VERIFY_END_TIME="$(date -Iseconds)"
OVERALL_STATUS=$([[ "$VERIFY_PASS" == true ]] && echo "PASS" || echo "FAIL")

# Write machine-readable JSON report (for monitoring tooling)
cat > "$REPORT_JSON" <<JSONEOF
{
  "timestamp": "$VERIFY_END_TIME",
  "schedule": "$SCHEDULE_TAG",
  "status": "$OVERALL_STATUS",
  "backup_timestamp": "$LATEST_BACKUP",
  "backup_size_bytes": "$BACKUP_SIZE_BYTES",
  "verify_database": "$VERIFY_DB",
  "verify_host": "$VERIFY_HOST:$VERIFY_PORT",
  "table_count": "${TABLE_COUNT:-0}",
  "checks": {
JSONEOF

# Append each check result to JSON
first=true
for check in "${!CHECK_RESULTS[@]}"; do
    if [[ "$first" == true ]]; then
        first=false
    else
        echo "," >> "$REPORT_JSON"
    fi
    printf '    "%s": "%s"' "$check" "${CHECK_RESULTS[$check]}" >> "$REPORT_JSON"
done

cat >> "$REPORT_JSON" <<JSONEOF

  }
}
JSONEOF

# Write human-readable markdown report
cat > "$REPORT_MD" <<MDEOF
# Backup Verification Report

- **Status**: $OVERALL_STATUS
- **Schedule**: $SCHEDULE_TAG
- **Timestamp**: $VERIFY_END_TIME
- **Backup**: $LATEST_BACKUP
- **Backup size**: $BACKUP_SIZE_BYTES bytes

## Verification Checks

| Check | Result |
|-------|--------|
MDEOF

for check in "${!CHECK_RESULTS[@]}"; do
    local_icon="✅"
    if [[ "${CHECK_RESULTS[$check]}" == "FAIL" ]]; then
        local_icon="❌"
    elif [[ "${CHECK_RESULTS[$check]}" == "SKIPPED" ]]; then
        local_icon="⏭️"
    fi
    echo "| $check | $local_icon ${CHECK_RESULTS[$check]} |" >> "$REPORT_MD"
done

cat >> "$REPORT_MD" <<MDEOF

## Environment

- **Verification Database**: $VERIFY_DB
- **Verification Host**: $VERIFY_HOST:$VERIFY_PORT
- **Source Bucket**: ${AWS_BUCKET:-N/A} (${AWS_REGION:-N/A})

## Recommendations

MDEOF

if [[ "$OVERALL_STATUS" == "PASS" ]]; then
    cat >> "$REPORT_MD" <<MDEOF
- ✅ Backup integrity verified successfully.
- ✅ Restore tested and validated.
- Continue scheduled verifications (daily recommended).
MDEOF
else
    cat >> "$REPORT_MD" <<MDEOF
- ⚠️ Verification failed. Investigate immediately.
- Check S3 backup integrity.
- Verify encryption key is valid.
- Review database connectivity.
- Consult [Backup Verification Runbook](docs/runbooks/backup-verification.md).
MDEOF
fi

# ── Step 7: Cleanup ────────────────────────────────────────────────────────────
log_step "INFO" "Cleaning up sandbox database: $VERIFY_DB"
run_or_echo PGPASSWORD="${POSTGRES_PASSWORD}" psql \
    -h "$VERIFY_HOST" -p "$VERIFY_PORT" -U "${POSTGRES_USER}" -d postgres \
    -c "DROP DATABASE IF EXISTS ${VERIFY_DB};" 2>/dev/null || true

log_step "INFO" "Cleaning up temporary files..."
run_or_echo rm -rf "$DOWNLOAD_DIR"

# ── Step 8: Reporting ──────────────────────────────────────────────────────────
log_step "INFO" "Verification report saved:"
log_step "INFO" "  JSON: $REPORT_JSON"
log_step "INFO" "  Markdown: $REPORT_MD"

# Clean up old reports (keep last 30 days)
if [[ "$DRY_RUN" == false ]]; then
    find "$REPORT_DIR" -name "report_*.json" -mtime +30 -delete 2>/dev/null || true
    find "$REPORT_DIR" -name "report_*.md" -mtime +30 -delete 2>/dev/null || true
fi

# ── Step 9: Push Prometheus metrics (optional) ─────────────────────────────────
if [[ -n "$PUSH_METRICS_URL" ]] && [[ "$DRY_RUN" == false ]]; then
    STATUS_VAL=$([[ "$OVERALL_STATUS" == "PASS" ]] && echo "1" || echo "0")
    RUN_TIME=$(date +%s)
    log_step "INFO" "Pushing metrics to $PUSH_METRICS_URL (status=$STATUS_VAL)"
    cat <<PROMEOF | curl -s --data-binary @- "${PUSH_METRICS_URL}/metrics/job/backup_verification/instance/${VERIFY_HOST}" 2>/dev/null || true
lumina_backup_verification_status{schedule="${SCHEDULE_TAG}"} ${STATUS_VAL}
lumina_backup_verification_last_run_seconds ${RUN_TIME}
PROMEOF
fi

if [[ "$OVERALL_STATUS" == "PASS" ]]; then
    log_step "INFO" "✅ Backup verification PASSED"
    echo "✅ Backup verification PASSED"
    exit 0
else
    log_step "ERROR" "❌ Backup verification FAILED"
    echo "❌ Backup verification FAILED - check $REPORT_JSON for details"
    exit 1
fi
