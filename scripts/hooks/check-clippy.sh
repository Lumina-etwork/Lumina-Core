#!/usr/bin/env bash
# Runs clippy only on the packages that own staged .rs files.
# Avoids a full-workspace clippy when only one crate changed.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

STAGED_RS=$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$' || true)
if [[ -z "$STAGED_RS" ]]; then exit 0; fi

# Derive affected packages from Cargo manifest membership
PKGS=$(cargo metadata --no-deps --format-version 1 2>/dev/null | \
  python3 -c "
import json, sys, os
meta = json.load(sys.stdin)
staged = sys.argv[1:]
pkgs = set()
for pkg in meta['packages']:
    src = os.path.dirname(pkg['manifest_path'])
    for f in staged:
        if os.path.abspath(f).startswith(os.path.abspath(src)):
            pkgs.add(pkg['name'])
print('\n'.join(pkgs))
" $STAGED_RS 2>/dev/null || true)

if [[ -z "$PKGS" ]]; then
  # Staged files outside workspace packages (e.g. root src/*.rs)
  cargo clippy --workspace --all-targets -- -D warnings
else
  for pkg in $PKGS; do
    cargo clippy -p "$pkg" --all-targets -- -D warnings
  done
fi
