#!/usr/bin/env bash
# Builds only the contract packages that have staged changes.
# Avoids unnecessary WASM rebuilds for unrelated commits.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

STAGED=$(git diff --cached --name-only --diff-filter=ACMR)
CONTRACTS=(grant_contracts staking_contract deposit_to_yield_adapter insurance_treasury)
FAILED=0

for contract in "${CONTRACTS[@]}"; do
  if echo "$STAGED" | grep -q "contracts/${contract}/"; then
    printf '[check-wasm-build] Building %s...\n' "$contract" >&2
    if ! cargo build -p "$contract" --target wasm32-unknown-unknown --release --quiet; then
      printf '[check-wasm-build] FAILED: %s\n' "$contract" >&2
      FAILED=1
    fi
  fi
done

exit $FAILED
