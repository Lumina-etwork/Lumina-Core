#!/usr/bin/env bash
# Checks only staged .rs files for formatting issues.
# Falls back to cargo fmt --check if rustfmt binary not available directly.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

STAGED_RS=$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$' || true)
if [[ -z "$STAGED_RS" ]]; then exit 0; fi

if command -v rustfmt &>/dev/null; then
  echo "$STAGED_RS" | xargs rustfmt --edition 2021 --check 2>&1
else
  cargo fmt --all -- --check
fi
