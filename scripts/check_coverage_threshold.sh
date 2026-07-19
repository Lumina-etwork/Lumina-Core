#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: check_coverage_threshold.sh <summary-file> [threshold-percent]

Reads a cargo-llvm-cov text summary and fails when line coverage is below the
threshold. The threshold defaults to COVERAGE_THRESHOLD, or 80 when unset.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

summary_file="${1:-target/llvm-cov/coverage-summary.txt}"
threshold="${2:-${COVERAGE_THRESHOLD:-80}}"

if [[ ! -f "$summary_file" ]]; then
  echo "coverage summary not found: $summary_file" >&2
  exit 2
fi

if ! [[ "$threshold" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "coverage threshold must be a percentage number, got: $threshold" >&2
  exit 2
fi

line_rate=$(
  awk '
    $1 == "TOTAL" {
      for (i = NF; i >= 1; i--) {
        if ($i ~ /^[0-9]+([.][0-9]+)?%$/) {
          gsub(/%/, "", $i)
          print $i
          found = 1
          exit
        }
      }
    }
    END { if (!found) exit 1 }
  ' "$summary_file"
) || {
  echo "unable to parse TOTAL line coverage percentage from $summary_file" >&2
  exit 2
}

awk -v actual="$line_rate" -v required="$threshold" 'BEGIN { exit (actual + 0 >= required + 0) ? 0 : 1 }' || {
  printf 'coverage %.2f%% is below required %.2f%%\n' "$line_rate" "$threshold" >&2
  exit 1
}

printf 'coverage %.2f%% meets required %.2f%%\n' "$line_rate" "$threshold"
