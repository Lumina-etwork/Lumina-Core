#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/setup-local-dev.sh [--check] [--dry-run] [--skip-build] [--skip-tests]

Bootstraps a local Lumina-Core development environment by validating required
tools, installing the wasm32 target when needed, and optionally running build
and test checks.

Options:
  --check       Validate prerequisites only; do not install targets or run checks.
  --dry-run     Print commands that would run without changing the system.
  --skip-build  Do not run the release WASM build.
  --skip-tests  Do not run cargo test.
  -h, --help    Show this help text.
USAGE
}

CHECK_ONLY=false
DRY_RUN=false
SKIP_BUILD=false
SKIP_TESTS=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) CHECK_ONLY=true ;;
    --dry-run) DRY_RUN=true ;;
    --skip-build) SKIP_BUILD=true ;;
    --skip-tests) SKIP_TESTS=true ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

log() { printf '[setup-local-dev] %s\n' "$*"; }
warn() { printf '[setup-local-dev] warning: %s\n' "$*" >&2; }
fail() { printf '[setup-local-dev] error: %s\n' "$*" >&2; exit 1; }

run() {
  if [[ "$DRY_RUN" == true ]]; then
    printf '[setup-local-dev] dry-run:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

require_command() {
  local name="$1"
  local hint="$2"
  if ! command -v "$name" >/dev/null 2>&1; then
    fail "missing required command '$name'. $hint"
  fi
}

repo_root() {
  local script_dir
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  cd -- "$script_dir/.." && pwd
}

ROOT="$(repo_root)"
cd "$ROOT"

log "checking required toolchain"
require_command cargo "Install Rust from https://rustup.rs/."
require_command rustc "Install Rust from https://rustup.rs/."
require_command rustup "Install rustup from https://rustup.rs/ so targets can be managed."

if command -v stellar >/dev/null 2>&1; then
  log "stellar CLI found: $(stellar --version 2>/dev/null | head -n 1)"
elif command -v soroban >/dev/null 2>&1; then
  log "soroban CLI found: $(soroban --version 2>/dev/null | head -n 1)"
else
  warn "Stellar/Soroban CLI not found. Install it before contract deployment workflows."
fi

if rustup target list --installed | sed 's/[[:space:]]*$//' | grep -Fxq 'wasm32-unknown-unknown'; then
  log "wasm32-unknown-unknown target already installed"
elif [[ "$CHECK_ONLY" == true ]]; then
  fail "missing Rust target wasm32-unknown-unknown. Run: rustup target add wasm32-unknown-unknown"
else
  log "installing wasm32-unknown-unknown target"
  run rustup target add wasm32-unknown-unknown
fi

if [[ "$CHECK_ONLY" == true ]]; then
  log "prerequisite check complete"
  exit 0
fi

if [[ "$SKIP_BUILD" == false ]]; then
  log "building default smart contracts for wasm32-unknown-unknown"
  run cargo build --target wasm32-unknown-unknown --release
else
  log "skipping build"
fi

if [[ "$SKIP_TESTS" == false ]]; then
  log "running workspace tests"
  run cargo test
else
  log "skipping tests"
fi

log "local development setup complete"
