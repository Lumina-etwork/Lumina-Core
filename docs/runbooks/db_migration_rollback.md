# Runbook: Database Migration Rollback

## Overview
This runbook details the steps to safely rollback a database migration in the event of failure, performance degradation, or data corruption.

## Prerequisites
- Access to production database credentials.
- Execution rights for `db_migration` tooling.

## Rollback Steps
1. **Identify the Target Version**: Determine the last known good version of the database schema.
2. **Execute Rollback**: Use the `rollback_to(version)` function from the `db_migration` module.
3. **Verify Integrity**: Run automated health checks to ensure data consistency.
4. **Monitor P99 Latency**: Ensure the 100ms P99 target is met post-rollback.
5. **Escalation**: If rollback fails, contact the Database Reliability Engineering (DBRE) team immediately.
