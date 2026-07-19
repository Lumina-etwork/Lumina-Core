#!/usr/bin/env bash
set -euo pipefail

BLUEPRINT="${1:-docs/chaos-engineering-staging.md}"

required_terms=(
  "staging only"
  "100 ms P99"
  "99.99%"
  "Security review checklist"
  "Blue-green and canary analysis"
  "Runbook template"
  "chaos-rpc-latency"
  "chaos-relay-partition"
  "chaos-db-failover"
  "chaos-worker-crash"
  "chaos-contract-call-revert"
)

if [[ ! -f "$BLUEPRINT" ]]; then
  echo "Missing chaos blueprint: $BLUEPRINT" >&2
  exit 1
fi

for term in "${required_terms[@]}"; do
  if ! rg --fixed-strings --quiet "$term" "$BLUEPRINT"; then
    echo "Chaos blueprint is missing required term: $term" >&2
    exit 1
  fi
done

echo "Chaos blueprint validation passed: $BLUEPRINT"
