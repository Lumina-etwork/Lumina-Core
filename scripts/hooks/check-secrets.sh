#!/usr/bin/env bash
# Runs gitleaks on the staged diff only.
# Degrades gracefully if gitleaks is not installed (soft failure).
set -euo pipefail

if ! command -v gitleaks &>/dev/null; then
  printf '[check-secrets] gitleaks not found — install via: brew install gitleaks\n' >&2
  printf '[check-secrets] WARNING: secret scan skipped; install gitleaks for local protection\n' >&2
  exit 0
fi

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

CONFIG_ARGS=()
if [[ -f "$REPO_ROOT/.gitleaks.toml" ]]; then
  CONFIG_ARGS=(--config "$REPO_ROOT/.gitleaks.toml")
fi

git diff --cached | gitleaks detect --pipe --no-banner --redact "${CONFIG_ARGS[@]}" 2>/dev/null
