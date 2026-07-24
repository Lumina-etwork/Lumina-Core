# Backup Verification Runbook

## Overview

The backup verification system (`scripts/verify_backup.sh`) performs scheduled,
automated database backup verification with restore testing. It is designed to
run as a cron job (recommended: daily at 03:00 UTC) and validates that
encrypted S3 backups can be successfully decrypted, restored, and queried.

## Architecture

```
┌──────────┐     ┌──────────────┐     ┌──────────┐
│   Cron   │────▶│ verify_backup│────▶│   AWS S3 │
│ Scheduler│     │    .sh       │     │ (backups)│
└──────────┘     └──────┬───────┘     └──────────┘
                        │
                        ▼
                 ┌──────────────┐
                 │   Sandbox    │
                 │  PostgreSQL  │
                 │  (verify DB) │
                 └──────┬───────┘
                        │
                        ▼
                 ┌──────────────┐
                 │  Integrity   │
                 │   Checks     │
                 └──────┬───────┘
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
    ┌──────────┐ ┌──────────┐ ┌───────────┐
    │  JSON    │ │ Markdown │ │Prometheus │
    │  Report  │ │  Report  │ │  Metrics  │
    └──────────┘ └──────────┘ └─────┬─────┘
                                    │
                                    ▼
                             ┌───────────┐
                             │PagerDuty  │
                             │  Alerts   │
                             └───────────┘
```

## Verification Checks

The script performs the following checks against restored backups:

1. **Database Connectivity** — verifies the restored database is accessible
2. **Table Count** — ensures at least the minimum expected number of tables
3. **Schema Validation** — compares restored tables against `social/db/schema.sql`
   and `analytics/db/schema.sql` CREATE TABLE definitions
4. **Row Count Sampling** — samples row counts for the first 10 tables
5. **Required Schemas** — validates presences of schemas specified in
   `VERIFY_REQUIRED_SCHEMAS` env var (optional)

## Scheduling

### Recommended Cron Configuration

```cron
# Daily backup verification at 03:00 UTC
0 3 * * * /opt/lumina-core/scripts/verify_backup.sh --schedule daily >> /var/log/lumina/verify_backup.log 2>&1
```

### Metrics Push (for Prometheus monitoring)

After each verification run, push metrics to your pushgateway:

```bash
STATUS=$?  # 0=pass, 1=fail
echo "lumina_backup_verification_status $STATUS" | curl --data-binary @- \
    "${PROMETHEUS_PUSHGATEWAY_URL}/metrics/job/backup_verification"
```

## Monitoring and Alerting

### Prometheus Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `lumina_backup_verification_status` | Gauge | 1=pass, 0=fail |
| `lumina_backup_verification_last_run_seconds` | Gauge | Unix timestamp of last run |

### Alerts

| Alert | Severity | Condition |
|-------|----------|-----------|
| `LuminaBackupVerificationFailed` | critical | Verification status is 0 for > 10 minutes |
| `LuminaBackupVerificationMissing` | warning | No verification completed in > 25 hours |

Both alerts are configured in `monitoring/incident_response_alerts.yaml`.

## Configuration

All configuration is sourced from `.env.backup` (or `VERIFY_BACKUP_CONFIG`):

| Variable | Required | Description |
|----------|----------|-------------|
| `POSTGRES_HOST` | Yes | PostgreSQL host for sandbox restore |
| `POSTGRES_PORT` | Yes | PostgreSQL port |
| `POSTGRES_DB` | Yes | Source database name |
| `POSTGRES_USER` | Yes | PostgreSQL user |
| `POSTGRES_PASSWORD` | Yes | PostgreSQL password |
| `AWS_BUCKET` | Yes | S3 bucket containing encrypted backups |
| `AWS_REGION` | Yes | AWS region |
| `AWS_ACCESS_KEY_ID` | Yes | AWS access key |
| `AWS_SECRET_ACCESS_KEY` | Yes | AWS secret key |
| `ENCRYPTION_KEY` | Yes | AES-256 encryption key (base64) |
| `VERIFY_DB` | No | Sandbox DB name (default: `lumina_verify_<PID>`) |
| `VERIFY_HOST` | No | Override sandbox host (default: `$POSTGRES_HOST`) |
| `VERIFY_PORT` | No | Override sandbox port (default: `$POSTGRES_PORT`) |
| `VERIFY_MIN_TABLE_COUNT` | No | Minimum expected table count (default: 1) |
| `VERIFY_REQUIRED_SCHEMAS` | No | Comma-separated required schema names |

## Troubleshooting

### Verification failed: No backups found in S3

- Check S3 bucket permissions
- Verify `backup_database.sh` is running successfully
- Confirm `AWS_BUCKET` and `AWS_REGION` are correct
- Check S3 lifecycle rules aren't prematurely expiring backups

### Verification failed: Decryption failed

- Verify `ENCRYPTION_KEY` matches the key used during backup
- Check for backup file corruption in S3
- Verify the backup file size in S3 metadata

### Verification failed: SQL_ERROR

- Check PostgreSQL connectivity from the verification host
- Verify `POSTGRES_USER` has `CREATEDB` privileges
- Check disk space on the sandbox PostgreSQL instance
- Verify the sandbox database doesn't already exist

### Verification failed: Table count below minimum

- Increase `VERIFY_MIN_TABLE_COUNT` if needed
- Check if the backup is from a newer deployment with fewer tables
- Verify the backup was taken from the correct database

## Deployment Strategy

### Blue-Green Rollout

1. **Canary**: Deploy `verify_backup.sh` to staging and run with `--dry-run` first
2. **Shadow**: Run live verification on staging environment for 1 week
3. **Production Canary**: Enable with `--schedule daily` on a single production
   host, monitoring alerts in silent mode
4. **Full Rollout**: Enable on all production hosts, arm PagerDuty alerts

### Validation Checklist

- [ ] `.env.backup` is properly configured on target host
- [ ] Sandbox PostgreSQL instance has sufficient disk space
- [ ] Cron job is configured and tested with `--dry-run`
- [ ] Prometheus pushgateway metrics are flowing
- [ ] PagerDuty alerts are configured and tested
- [ ] First live verification run passes
- [ ] Verification reports are being generated in `backups/verification/`

## Security Review

- The verification script reads encrypted backups from S3 and decrypts them
  using the same `ENCRYPTION_KEY` used for backup. Ensure this key is
  stored securely (e.g., in a secrets manager or restricted `.env.backup`).
- The sandbox database is created and destroyed within the same script run.
  No persistent copy of decrypted data remains after verification.
- All report files are stored locally in `backups/verification/` and
  cleaned up after 30 days.
- No backup data is transmitted outside of the S3 → sandbox → cleanup
  pipeline.

## Related Documentation

- [Backup Script](../scripts/backup_database.sh)
- [Recovery Script](../scripts/recover_database.sh)
- [Fire Drill Script](../scripts/fire_drill.sh)
- [Multi-Region DR Test](../scripts/multi_region_dr_test.sh)
- [Incident Response PagerDuty](../operations/incident-response-pagerduty.md)
