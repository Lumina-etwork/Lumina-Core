# Database Migration Runbook

## Architecture

Database migrations are versioned per service in `db/migrations/<service>`. The runner stores applied versions in `public.schema_migrations`, validates file checksums, and serializes operations with a PostgreSQL advisory lock. Forward and rollback files are reviewed together so every schema change has an operational recovery path.

## Pre-deployment checklist

1. Confirm the application change is backward compatible with both the current and target schema.
2. Run migration validation in CI.
3. Confirm dashboard panels for migration duration, failure count, checksum drift, lock waits, and database availability are healthy.
4. Verify backups are recent and restorable with `scripts/fire_drill.sh`.

## Blue-green and canary flow

1. Keep the current green environment serving 100% traffic.
2. Apply migrations to the shared database with `scripts/migrate_database.sh up <service>`.
3. Deploy the blue environment with the new application version.
4. Send 1%, 10%, 50%, then 100% traffic to blue if canary metrics remain healthy for the agreed window.
5. Keep green warm until the post-deploy verification window closes.

## Rollback flow

1. Shift traffic back to green immediately if availability, latency, error rate, or security monitors breach policy.
2. If the old application cannot safely run against the migrated schema, execute `scripts/migrate_database.sh down <service> <target_version>`.
3. Validate `scripts/migrate_database.sh status <service>` and application health checks.
4. Open an incident review if rollback was caused by checksum drift, failed DDL, data loss risk, or availability impact.

## Security review notes

- Use a dedicated migration role with DDL privileges; runtime roles should not own migration DDL permissions.
- Never include secrets in migration files.
- Checksum mismatch is considered tampering or an unreviewed production edit until proven otherwise.
