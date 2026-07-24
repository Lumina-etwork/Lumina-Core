#!/usr/bin/env bash
# ci-local.sh — Ejecuta los mismos checks del CI en tu máquina local
# Uso: ./scripts/ci-local.sh [check|contracts|security|docker|all]
# Por defecto corre 'all'

set -euo pipefail

MODE="${1:-all}"
PASS=0
FAIL=0

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; PASS=$((PASS+1)); }
fail() { echo -e "${RED}✗ $1${NC}"; FAIL=$((FAIL+1)); }
header() { echo -e "\n${YELLOW}── $1 ──${NC}"; }

# ── LINT ────────────────────────────────────────────────────────────────────
run_lint() {
  header "Lint"

  if cargo fmt --all -- --check 2>/dev/null; then
    pass "rustfmt"
  else
    fail "rustfmt (corre: cargo fmt --all)"
  fi

  if cargo clippy --workspace --all-targets -- -D warnings 2>/dev/null; then
    pass "clippy"
  else
    fail "clippy"
  fi
}

# ── CORE INFRASTRUCTURE ──────────────────────────────────────────────────────
run_core() {
  header "Core Infrastructure"

  for crate in lumina-consensus lumina-audit core-engine; do
    if cargo test -p "$crate" --quiet 2>/dev/null; then
      pass "tests: $crate"
    else
      fail "tests: $crate"
    fi
  done
}

# ── SMART CONTRACTS ──────────────────────────────────────────────────────────
run_contracts() {
  header "Smart Contracts (workspace)"

  for crate in grant_contracts staking_contract deposit_to_yield_adapter insurance_treasury; do
    if cargo build -p "$crate" --target wasm32-unknown-unknown --release --quiet 2>/dev/null; then
      pass "wasm build: $crate"
    else
      fail "wasm build: $crate"
    fi

    if cargo test -p "$crate" --quiet 2>/dev/null; then
      pass "tests: $crate"
    else
      fail "tests: $crate"
    fi
  done
}

# ── BACKEND SERVICES ─────────────────────────────────────────────────────────
run_backends() {
  header "Backend Services"

  for service in analytics social; do
    if [ -d "$service" ]; then
      if (cd "$service" && cargo test --quiet 2>/dev/null); then
        pass "tests: $service"
      else
        fail "tests: $service"
      fi
    else
      echo "  skipping $service (directory not found)"
    fi
  done
}

# ── DOCKER LAYER CACHE BUILD ─────────────────────────────────────────────────
run_docker() {
  header "Docker Layer Cache Build"

  if ! command -v docker &>/dev/null; then
    echo "  skipping docker build (docker no instalado)"
    return
  fi

  DOCKER_CACHE_DIR="${HOME}/.cache/lumina-docker-build"
  mkdir -p "$DOCKER_CACHE_DIR"

  TARGET="${DOCKER_TARGET:-build}"
  START=$(date +%s)

  if DOCKER_BUILDKIT=1 docker build \
      --target "$TARGET" \
      --cache-from "type=local,src=${DOCKER_CACHE_DIR}" \
      --cache-to "type=local,dest=${DOCKER_CACHE_DIR},mode=max" \
      --build-arg "RUST_VERSION=${RUST_VERSION:-1.91.0}" \
      --build-arg "STELLAR_CLI_VERSION=${STELLAR_CLI_VERSION:-22.1.0}" \
      -t "lumina-core:local" \
      . 2>/dev/null; then
    END=$(date +%s)
    pass "docker build (target=${TARGET}, $((END-START))s)"
  else
    fail "docker build (target=${TARGET})"
  fi
}

# ── SECURITY ─────────────────────────────────────────────────────────────────
run_security() {
  header "Security"

  if command -v cargo-audit &>/dev/null; then
    if cargo audit --quiet 2>/dev/null; then
      pass "cargo audit"
    else
      fail "cargo audit (vulnerabilities found)"
    fi
  else
    echo "  skipping cargo-audit (not installed — corre: cargo install cargo-audit)"
  fi

  if [ -d "doc_tests" ]; then
    if (cd doc_tests && cargo test --quiet 2>/dev/null); then
      pass "security doc tests"
    else
      fail "security doc tests"
    fi
  fi
}

# ── HOOK TESTS ───────────────────────────────────────────────────────────────
run_hooks() {
  header "Pre-commit Hook Tests"

  if command -v bats &>/dev/null; then
    if bats tests/hooks/ --quiet 2>/dev/null; then
      pass "hook unit tests (bats)"
    else
      fail "hook unit tests (bats)"
    fi
  else
    echo "  skipping hook tests (bats not installed — brew install bats-core)"
  fi

  if command -v shellcheck &>/dev/null; then
    if shellcheck --severity=warning scripts/hooks/*.sh 2>/dev/null; then
      pass "hook shellcheck"
    else
      fail "hook shellcheck"
    fi
  else
    echo "  skipping shellcheck (not installed — brew install shellcheck)"
  fi
}

# ── MAIN ─────────────────────────────────────────────────────────────────────
case "$MODE" in
  check)
    run_lint
    ;;
  contracts)
    run_contracts
    ;;
  security)
    run_security
    ;;
  docker)
    run_docker
    ;;
  all)
    run_lint
    run_core
    run_contracts
    run_backends
    run_security
    run_docker
    ;;
  *)
    echo "Uso: $0 [check|contracts|security|docker|all]"
    exit 1
    ;;
esac

echo ""
echo -e "Resultado: ${GREEN}${PASS} pasaron${NC}  ${RED}${FAIL} fallaron${NC}"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
