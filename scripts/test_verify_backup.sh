#!/usr/bin/env bash
# tests for scripts/verify_backup.sh
#
# Validates the backup verification script's core logic paths without
# requiring actual S3 or PostgreSQL connectivity.
#
# Usage: ./scripts/test_verify_backup.sh

set -euo pipefail

SCRIPT_UNDER_TEST="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/verify_backup.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# ── Helpers ────────────────────────────────────────────────────────────────────

make_stub() {
    local name="$1"; local body="$2"
    cat > "$TMP_DIR/$name" <<'STUB'
#!/usr/bin/env bash
STUB
    echo "$body" >> "$TMP_DIR/$name"
    chmod +x "$TMP_DIR/$name"
}

assert_contains() {
    local haystack="$1"; local needle="$2"
    if [[ "$haystack" != *"$needle"* ]]; then
        printf 'Expected output to contain: %s\nActual output:\n%s\n' "$needle" "$haystack" >&2
        exit 1
    fi
}

assert_exit_code() {
    local expected="$1"; local actual="$2"; local context="$3"
    if [[ "$actual" != "$expected" ]]; then
        printf '[%s] Expected exit code %d, got %d\n' "$context" "$expected" "$actual" >&2
        exit 1
    fi
}

# ── Setup fake .env.backup ─────────────────────────────────────────────────────
cat > "$TMP_DIR/.env.backup" <<'ENVEOF'
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DB=lumina_core
POSTGRES_USER=lumina
POSTGRES_PASSWORD=testpass
AWS_BUCKET=lumina-dr-backups
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=AKIATEST
AWS_SECRET_ACCESS_KEY=testsecret
ENCRYPTION_KEY=testkey123
ENVEOF

export VERIFY_BACKUP_CONFIG="$TMP_DIR/.env.backup"

# ── Test 1: --help prints usage ────────────────────────────────────────────────
test_help_flag() {
    local output
    output="$("$SCRIPT_UNDER_TEST" --help 2>&1)"
    assert_contains "$output" "Usage:"
    assert_contains "$output" "Scheduled Database Backup Verification"
    echo "  PASS: --help flag"
}

# ── Test 2: --dry-run exits 0 with valid config ────────────────────────────────
test_dry_run_with_config() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --schedule daily 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "dry-run"
    assert_contains "$output" "DRY RUN"
    assert_contains "$output" "Backup verification PASSED"
    echo "  PASS: dry-run mode exits 0"
}

# ── Test 3: --dry-run respects --schedule tag ──────────────────────────────────
test_dry_run_schedule_tag() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --schedule hourly 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "dry-run-hourly"
    assert_contains "$output" "hourly"
    echo "  PASS: --schedule hourly tag in output"
}

# ── Test 4: Missing config exits non-zero (live mode) ──────────────────────────
test_missing_config_live_mode() {
    local output
    set +e
    output="$(VERIFY_BACKUP_CONFIG=/nonexistent/file "$SCRIPT_UNDER_TEST" 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 1 "$exit_code" "missing-config"
    assert_contains "$output" "not found"
    echo "  PASS: missing config exits 1 in live mode"
}

# ── Test 5: Unknown flag exits non-zero ────────────────────────────────────────
test_unknown_flag() {
    local output
    set +e
    output="$("$SCRIPT_UNDER_TEST" --definitely-not-a-real-flag 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 1 "$exit_code" "unknown-flag"
    assert_contains "$output" "Unknown argument"
    echo "  PASS: unknown flag exits 1"
}

# ── Test 6: --dry-run with --verify-db flag ────────────────────────────────────
test_dry_run_custom_verify_db() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --verify-db my_sandbox 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "custom-verify-db"
    assert_contains "$output" "my_sandbox"
    echo "  PASS: custom --verify-db flag"
}

# ── Test 7: Dry-run output includes verification steps ────────────────────────
test_dry_run_output_verification_steps() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'
    make_stub jq 'echo "1024000"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --schedule daily 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "dry-run-output"
    assert_contains "$output" "Dry-run mode: skipping live database integrity checks"
    assert_contains "$output" "Static schema check"
    assert_contains "$output" "Verification report saved"
    echo "  PASS: dry-run output includes all verification steps"
}

# ── Test 8: Dry-run with no S3 backup still passes in dry-run ──────────────────
test_dry_run_missing_backup_still_passes() {
    # aws stub returns no output (simulating empty S3)
    make_stub aws 'exit 0'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'
    make_stub jq 'echo "0"'

    # Even though aws returns nothing, dry-run mode uses LATEST_BACKUP_MOCK or
    # would fail in live mode. Let's verify live mode fails properly by
    # temporarily overriding the config to simulate empty S3.
    # Actually, in dry-run mode we set LATEST_BACKUP to a mock value so it
    # should pass. This test validates that behavior.

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" LATEST_BACKUP_MOCK="20250724_000000" "$SCRIPT_UNDER_TEST" --dry-run 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "dry-run-mock-backup"
    assert_contains "$output" "Backup verification PASSED"
    echo "  PASS: dry-run handles mocked backup timestamp"
}

# ── Test 9: --push-metrics flag accepted ──────────────────────────────────────
test_push_metrics_flag() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'
    make_stub jq 'echo "0"'
    make_stub curl 'echo "curl-stub $@"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --push-metrics http://pushgateway:9091 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "push-metrics-flag"
    # In dry-run mode, metrics push is skipped, but flag should be accepted
    assert_contains "$output" "Backup verification PASSED"
    echo "  PASS: --push-metrics flag accepted"
}

# ── Test 10: Cleanup logic doesn't crash ──────────────────────────────────────
test_missing_required_env_vars() {
    # Create a partial config missing POSTGRES_HOST
    local partial_config="$TMP_DIR/.env.partial"
    cat > "$partial_config" <<'ENVEOF'
POSTGRES_PORT=5432
POSTGRES_DB=lumina_core
POSTGRES_USER=lumina
ENVEOF

    # This should work for dry-run but fail for live
    local output
    set +e
    output="$(VERIFY_BACKUP_CONFIG="$partial_config" "$SCRIPT_UNDER_TEST" --dry-run 2>&1)"
    local exit_code=$?
    set -e

    # Dry-run should still pass with partial config
    assert_exit_code 0 "$exit_code" "partial-config-dry-run"
    echo "  PASS: dry-run tolerant of partial config"
}

# ── Test 11: Cleanup logic doesn't crash ──────────────────────────────────────
test_cleanup_logic_no_crash() {
    make_stub aws 'echo "aws-stub $@"'
    make_stub psql 'echo "psql-stub $@"'
    make_stub pg_restore 'echo "pg_restore-stub $@"'
    make_stub jq 'echo "0"'

    local output
    set +e
    output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run --schedule daily 2>&1)"
    local exit_code=$?
    set -e

    assert_exit_code 0 "$exit_code" "cleanup-dry-run"
    assert_contains "$output" "Cleaning up sandbox database"
    assert_contains "$output" "Cleaning up temporary files"
    echo "  PASS: cleanup logic doesn't crash"
}

# ── Run all tests ──────────────────────────────────────────────────────────────
echo "Running verify_backup.sh tests..."

test_help_flag
test_dry_run_with_config
test_dry_run_schedule_tag
test_missing_config_live_mode
test_unknown_flag
test_dry_run_custom_verify_db
test_dry_run_output_verification_steps
test_dry_run_missing_backup_still_passes
test_push_metrics_flag
test_missing_required_env_vars
test_cleanup_logic_no_crash

echo ""
echo "✅ All verify_backup.sh tests passed"
