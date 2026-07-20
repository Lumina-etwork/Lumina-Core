#!/usr/bin/env bats
# tests/hooks/pre-commit.bats
# Unit tests for the Lumina-Core pre-commit hook suite.
# Install bats: brew install bats-core  |  apt-get install bats

setup() {
  REPO_ROOT="$(git rev-parse --show-toplevel)"
  HOOKS_DIR="$REPO_ROOT/scripts/hooks"
  TMPDIR_BASE=$(mktemp -d)
  export TMPDIR_BASE
  git init "$TMPDIR_BASE/test-repo" --quiet
  git -C "$TMPDIR_BASE/test-repo" config user.email "test@lumina.local"
  git -C "$TMPDIR_BASE/test-repo" config user.name "Test"
  # Initial commit so HEAD exists
  touch "$TMPDIR_BASE/test-repo/.gitkeep"
  git -C "$TMPDIR_BASE/test-repo" add .gitkeep
  git -C "$TMPDIR_BASE/test-repo" commit -m "chore: init" --quiet
}

teardown() {
  rm -rf "$TMPDIR_BASE"
}

# ── check-file-guards ──────────────────────────────────────────────────────────

@test "check-file-guards: blocks .env files" {
  cd "$TMPDIR_BASE/test-repo"
  echo "SECRET=abc123" > .env
  git add .env
  run bash "$HOOKS_DIR/check-file-guards.sh"
  [ "$status" -eq 1 ]
  [[ "$output" =~ "BLOCKED" ]]
}

@test "check-file-guards: blocks .pem files" {
  cd "$TMPDIR_BASE/test-repo"
  echo "-----BEGIN CERTIFICATE-----" > cert.pem
  git add cert.pem
  run bash "$HOOKS_DIR/check-file-guards.sh"
  [ "$status" -eq 1 ]
}

@test "check-file-guards: blocks files over 5MB" {
  cd "$TMPDIR_BASE/test-repo"
  dd if=/dev/zero of=bigfile bs=1M count=6 2>/dev/null
  git add bigfile
  run bash "$HOOKS_DIR/check-file-guards.sh"
  [ "$status" -eq 1 ]
  [[ "$output" =~ "limit: 5MB" ]]
}

@test "check-file-guards: allows normal source files" {
  cd "$TMPDIR_BASE/test-repo"
  echo 'fn main() {}' > main.rs
  git add main.rs
  run bash "$HOOKS_DIR/check-file-guards.sh"
  [ "$status" -eq 0 ]
}

@test "check-file-guards: allows files exactly at 5MB limit" {
  cd "$TMPDIR_BASE/test-repo"
  dd if=/dev/zero of=exactly5mb bs=1M count=5 2>/dev/null
  git add exactly5mb
  run bash "$HOOKS_DIR/check-file-guards.sh"
  [ "$status" -eq 0 ]
}

# ── commit-msg ────────────────────────────────────────────────────────────────

@test "commit-msg: accepts feat type" {
  MSG_FILE=$(mktemp)
  echo "feat(staking): add auto-compound mechanism" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: accepts fix type" {
  MSG_FILE=$(mktemp)
  echo "fix(consensus): resolve leader election deadlock" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: accepts security type" {
  MSG_FILE=$(mktemp)
  echo "security(contracts): patch reentrancy in deposit adapter" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: accepts contract type" {
  MSG_FILE=$(mktemp)
  echo "contract(grant): add vesting schedule" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: accepts type without scope" {
  MSG_FILE=$(mktemp)
  echo "chore: update dependencies" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: rejects non-conventional message" {
  MSG_FILE=$(mktemp)
  echo "fixed a bug" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 1 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: rejects unknown type" {
  MSG_FILE=$(mktemp)
  echo "change(core): something" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 1 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: allows Merge commits" {
  MSG_FILE=$(mktemp)
  echo "Merge pull request #42 from feature/staking-compound" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: allows Revert commits" {
  MSG_FILE=$(mktemp)
  echo 'Revert "feat(consensus): add optimistic finality"' > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

@test "commit-msg: allows fixup commits" {
  MSG_FILE=$(mktemp)
  echo "fixup! feat(staking): add auto-compound mechanism" > "$MSG_FILE"
  run bash "$HOOKS_DIR/commit-msg" "$MSG_FILE"
  [ "$status" -eq 0 ]
  rm -f "$MSG_FILE"
}

# ── check-secrets ─────────────────────────────────────────────────────────────

@test "check-secrets: degrades gracefully without gitleaks" {
  HOOKS_DIR_LOCAL="$HOOKS_DIR"
  run env PATH="/nonexistent:$PATH" bash "$HOOKS_DIR_LOCAL/check-secrets.sh"
  [ "$status" -eq 0 ]
  [[ "$output" =~ "not found" ]]
}

# ── check-shellcheck ──────────────────────────────────────────────────────────

@test "check-shellcheck: degrades gracefully without shellcheck" {
  run env PATH="/nonexistent:$PATH" bash "$HOOKS_DIR/check-shellcheck.sh"
  [ "$status" -eq 0 ]
  [[ "$output" =~ "not found" ]]
}

# ── emit-hook-metrics ─────────────────────────────────────────────────────────

@test "emit-hook-metrics: exits 0 when pushgateway unreachable" {
  run env PROMETHEUS_PUSHGATEWAY_URL="http://127.0.0.1:19999" \
    bash "$HOOKS_DIR/emit-hook-metrics.sh" 3 0 250
  [ "$status" -eq 0 ]
}

@test "emit-hook-metrics: exits 0 with all-zero arguments" {
  run env PROMETHEUS_PUSHGATEWAY_URL="http://127.0.0.1:19999" \
    bash "$HOOKS_DIR/emit-hook-metrics.sh" 0 0 0
  [ "$status" -eq 0 ]
}
