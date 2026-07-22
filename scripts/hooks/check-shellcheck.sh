#!/usr/bin/env bash
# Runs shellcheck on staged shell scripts.
# Degrades gracefully if shellcheck is not installed (soft failure).
set -euo pipefail

if ! command -v shellcheck &>/dev/null; then
  printf '[check-shellcheck] shellcheck not found — install via: brew install shellcheck\n' >&2
  exit 0
fi

STAGED_SH=$(git diff --cached --name-only --diff-filter=ACMR | grep -E '\.(sh|bash)$' || true)
if [[ -z "$STAGED_SH" ]]; then exit 0; fi

echo "$STAGED_SH" | xargs shellcheck --severity=warning
