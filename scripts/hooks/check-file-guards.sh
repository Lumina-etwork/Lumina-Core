#!/usr/bin/env bash
# Blocks commits of accidentally large files (>5MB) and known sensitive file patterns.
set -euo pipefail

FAIL=0

# Large file detection (limit: 5MB)
while IFS= read -r file; do
  if [[ -f "$file" ]]; then
    size=$(wc -c < "$file")
    if (( size > 5242880 )); then
      printf '[file-guard] BLOCKED: %s is %dMB (limit: 5MB)\n' \
        "$file" "$((size / 1048576))" >&2
      FAIL=1
    fi
  fi
done < <(git diff --cached --name-only --diff-filter=ACMR)

# Deny-list: file patterns that should never be committed
DENYLIST=(
  '\.env$'
  '\.pem$'
  '\.key$'
  'id_rsa$'
  'id_ed25519$'
  '\.p12$'
  '\.pfx$'
  'secrets\.toml$'
  'credentials\.json$'
  '\.secret$'
)

for pattern in "${DENYLIST[@]}"; do
  matches=$(git diff --cached --name-only --diff-filter=ACMR | grep -E "$pattern" || true)
  if [[ -n "$matches" ]]; then
    printf '[file-guard] BLOCKED: staged file matches deny-list pattern (%s):\n%s\n' \
      "$pattern" "$matches" >&2
    FAIL=1
  fi
done

exit $FAIL
