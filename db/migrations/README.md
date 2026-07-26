# Database Migration Versioning and Rollback

Lumina services use ordered PostgreSQL migration files with explicit rollback support. Each service owns a directory under `db/migrations/<service>` and every migration is paired as:

- `NNNNNN_description.up.sql` for forward changes.
- `NNNNNN_description.down.sql` for rollback changes.

The runner records applied versions, SHA-256 checksums, execution time, and status in `public.schema_migrations`. A PostgreSQL advisory lock serializes migration activity per database so competing deployments cannot apply different versions concurrently.

## Operating model

1. Add compatible, backward-safe schema changes in `*.up.sql`.
2. Add the exact inverse in `*.down.sql` where rollback is safe.
3. Run `scripts/migrate_database.sh validate <service>` in CI to detect missing pairs or edited applied files.
4. During deployment, run `scripts/migrate_database.sh up <service>` before shifting traffic.
5. For rollback, first shift traffic back to the previous application version, then run `scripts/migrate_database.sh down <service> <target_version>`.

## Performance and availability controls

- Indexes are created concurrently in service migrations where possible.
- Lock and statement timeouts are set by the runner to avoid long critical-path blocking.
- Migrations run outside request paths and expose Prometheus metrics from `monitoring/database_migrations.prometheus.yml`.
- Blue-green and canary deployment steps are documented in `docs/runbooks/database_migrations.md`.

## Security controls

- Checksums make post-review migration edits visible.
- The runtime database role should be different from the migration role.
- Rollbacks are explicit SQL files and must be reviewed with their matching forward migration.
