#!/usr/bin/env bash
set -euo pipefail

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

cat > "$tmpdir/pass.txt" <<'SUMMARY'
Filename        Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover
TOTAL                100                10    90.00%          20                 1    95.00%         200                20    90.00%
SUMMARY

pass_output="$(scripts/check_coverage_threshold.sh "$tmpdir/pass.txt" 80)"
[[ "$pass_output" == *"coverage 90.00% meets required 80.00%"* ]]

cat > "$tmpdir/fail.txt" <<'SUMMARY'
Filename        Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover
TOTAL                100                30    70.00%          20                 2    90.00%         200                60    70.00%
SUMMARY

if scripts/check_coverage_threshold.sh "$tmpdir/fail.txt" 80 >"$tmpdir/fail.out" 2>&1; then
  echo "expected threshold failure" >&2
  exit 1
fi
rg -q "coverage 70.00% is below required 80.00%" "$tmpdir/fail.out"

cat > "$tmpdir/malformed.txt" <<'SUMMARY'
no total coverage here
SUMMARY

if scripts/check_coverage_threshold.sh "$tmpdir/malformed.txt" 80 >"$tmpdir/malformed.out" 2>&1; then
  echo "expected malformed summary failure" >&2
  exit 1
fi
rg -q "unable to parse TOTAL line coverage" "$tmpdir/malformed.out"

echo "coverage threshold script tests passed"
