#!/usr/bin/env bash
set -euo pipefail

SCRIPT_UNDER_TEST="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)/scripts/setup-local-dev.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

make_stub() {
  local name="$1"
  local body="$2"
  cat > "$TMP_DIR/$name" <<STUB
#!/usr/bin/env bash
set -euo pipefail
$body
STUB
  chmod +x "$TMP_DIR/$name"
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    printf 'Expected output to contain: %s\nActual output:\n%s\n' "$needle" "$haystack" >&2
    exit 1
  fi
}

test_check_mode_passes_when_wasm_target_installed() {
  make_stub cargo 'echo cargo-stub "$@"'
  make_stub rustc 'echo rustc-stub "$@"'
  make_stub rustup 'if [[ "$1 $2 $3" == "target list --installed" ]]; then echo wasm32-unknown-unknown; else echo rustup-stub "$@"; fi'
  local output
  output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --check 2>&1)"
  assert_contains "$output" "prerequisite check complete"
}

test_dry_run_prints_install_build_and_test_commands() {
  make_stub cargo 'echo cargo-stub "$@"'
  make_stub rustc 'echo rustc-stub "$@"'
  make_stub rustup 'if [[ "$1 $2 $3" == "target list --installed" ]]; then exit 0; fi; echo rustup-stub "$@"'
  local output
  output="$(PATH="$TMP_DIR:$PATH" "$SCRIPT_UNDER_TEST" --dry-run 2>&1)"
  assert_contains "$output" "dry-run: rustup target add wasm32-unknown-unknown"
  assert_contains "$output" "dry-run: cargo build --target wasm32-unknown-unknown --release"
  assert_contains "$output" "dry-run: cargo test"
}

test_unknown_option_fails() {
  if "$SCRIPT_UNDER_TEST" --definitely-not-real >/tmp/setup-local-dev-test.out 2>&1; then
    cat /tmp/setup-local-dev-test.out >&2
    exit 1
  fi
  assert_contains "$(cat /tmp/setup-local-dev-test.out)" "Unknown option"
}

test_check_mode_passes_when_wasm_target_installed
test_dry_run_prints_install_build_and_test_commands
test_unknown_option_fails
printf 'setup-local-dev tests passed\n'
