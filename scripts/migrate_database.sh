#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/migrate_database.sh <up|down|status|validate> <service> [target_version]

Environment:
  DATABASE_URL       PostgreSQL connection string (required except validate)
  MIGRATIONS_DIR     Override migration root (default: db/migrations)
  LOCK_TIMEOUT       PostgreSQL lock timeout (default: 5s)
  STATEMENT_TIMEOUT  PostgreSQL statement timeout (default: 60s)
USAGE
}

command="${1:-}"
service="${2:-}"
target_version="${3:-0}"
root="${MIGRATIONS_DIR:-db/migrations}"
lock_timeout="${LOCK_TIMEOUT:-5s}"
statement_timeout="${STATEMENT_TIMEOUT:-60s}"

if [[ -z "$command" || -z "$service" ]]; then
  usage >&2
  exit 64
fi

if [[ ! "$service" =~ ^[A-Za-z0-9_-]+$ ]]; then
  echo "service must contain only letters, numbers, underscores, or dashes" >&2
  exit 64
fi

if [[ ! "$target_version" =~ ^[0-9]+$ ]]; then
  echo "target_version must be numeric" >&2
  exit 64
fi

service_dir="$root/$service"
if [[ ! -d "$service_dir" ]]; then
  echo "unknown service migration directory: $service_dir" >&2
  exit 66
fi

sha256() {
  sha256sum "$1" | awk '{print $1}'
}

version_of() {
  basename "$1" | cut -d_ -f1
}

validate_pairs() {
  local failed=0 up down version stem
  shopt -s nullglob
  for up in "$service_dir"/*.up.sql; do
    version="$(version_of "$up")"
    stem="${up%.up.sql}"
    down="$stem.down.sql"
    if [[ ! "$version" =~ ^[0-9]{6}$ ]]; then
      echo "invalid migration version in $up" >&2
      failed=1
    fi
    if [[ ! -f "$down" ]]; then
      echo "missing rollback migration for $up" >&2
      failed=1
    fi
  done
  for down in "$service_dir"/*.down.sql; do
    if [[ ! -f "${down%.down.sql}.up.sql" ]]; then
      echo "missing forward migration for $down" >&2
      failed=1
    fi
  done
  return "$failed"
}

psql_exec() {
  PGOPTIONS="-c lock_timeout=$lock_timeout -c statement_timeout=$statement_timeout ${PGOPTIONS:-}" \
    psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -X "$@"
}

ensure_db() {
  if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "DATABASE_URL is required for $command" >&2
    exit 64
  fi
  psql_exec <<SQL
CREATE TABLE IF NOT EXISTS public.schema_migrations (
  service text NOT NULL,
  version integer NOT NULL,
  name text NOT NULL,
  checksum text NOT NULL,
  direction text NOT NULL CHECK (direction IN ('up', 'down')),
  execution_ms integer NOT NULL DEFAULT 0,
  applied_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (service, version)
);
SQL
}

case "$command" in
  validate)
    validate_pairs
    ;;
  status)
    ensure_db
    psql_exec -c "SELECT service, version, name, checksum, applied_at FROM public.schema_migrations WHERE service = '$service' ORDER BY version;"
    ;;
  up)
    validate_pairs
    ensure_db
    psql_exec -c "SELECT pg_advisory_lock(hashtext('lumina-migrations:' || '$service'));"
    trap "psql_exec -c \"SELECT pg_advisory_unlock(hashtext('lumina-migrations:' || '$service'));\" >/dev/null" EXIT
    shopt -s nullglob
    for file in "$service_dir"/*.up.sql; do
      version="$(version_of "$file")"
      name="$(basename "$file")"
      checksum="$(sha256 "$file")"
      applied="$(psql_exec -At -c "SELECT checksum FROM public.schema_migrations WHERE service = '$service' AND version = $((10#$version));")"
      if [[ -n "$applied" ]]; then
        if [[ "$applied" != "$checksum" ]]; then
          echo "checksum mismatch for applied migration $name" >&2
          exit 65
        fi
        continue
      fi
      start_ms="$(date +%s%3N)"
      psql_exec -f "$file"
      elapsed_ms=$(( $(date +%s%3N) - start_ms ))
      psql_exec -c "INSERT INTO public.schema_migrations(service, version, name, checksum, direction, execution_ms) VALUES ('$service', $((10#$version)), '$name', '$checksum', 'up', $elapsed_ms);"
    done
    ;;
  down)
    ensure_db
    psql_exec -c "SELECT pg_advisory_lock(hashtext('lumina-migrations:' || '$service'));"
    trap "psql_exec -c \"SELECT pg_advisory_unlock(hashtext('lumina-migrations:' || '$service'));\" >/dev/null" EXIT
    mapfile -t versions < <(psql_exec -At -c "SELECT version FROM public.schema_migrations WHERE service = '$service' AND version > $target_version ORDER BY version DESC;")
    for version in "${versions[@]}"; do
      file="$(printf '%s/%06d_' "$service_dir" "$version")"
      file="$(compgen -G "${file}*.down.sql" | head -n 1)"
      if [[ -z "$file" ]]; then
        echo "missing rollback for version $version" >&2
        exit 66
      fi
      psql_exec -f "$file"
      psql_exec -c "DELETE FROM public.schema_migrations WHERE service = '$service' AND version = $version;"
    done
    ;;
  *)
    usage >&2
    exit 64
    ;;
esac
