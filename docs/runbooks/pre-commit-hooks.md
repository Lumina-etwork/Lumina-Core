# Pre-Commit Hook Suite Runbook

## Architecture

The Lumina-Core pre-commit hook suite runs on each developer workstation at commit time.
Checks are staged-file-aware and run in parallel groups to minimize developer wait time.

Hook source files live in `scripts/hooks/` and are installed into `.git/hooks/` by
`scripts/setup-local-dev.sh`. The CI job `hook-tests` (BATS) validates hook correctness
on every PR.

## Checks performed

| Hook trigger | Check | Tool | Skip condition |
|---|---|---|---|
| pre-commit (always) | Secret detection | gitleaks | tool not installed (soft) |
| pre-commit (always) | Large file / deny-list | bash | none |
| pre-commit (`*.rs`, `*.toml` staged) | Formatting | rustfmt | no Rust files staged |
| pre-commit (`*.rs`, `*.toml` staged) | Lint | clippy -D warnings | no Rust files staged |
| pre-commit (`contracts/` staged) | WASM build | cargo | no contract files staged |
| pre-commit (`*.sh` staged) | Shell lint | shellcheck | tool not installed (soft) |
| pre-push | Dependency policy | cargo-deny | tool not installed (soft) |
| commit-msg | Conventional Commits format | bash regex | merge/revert commits |

## Performance targets

| Commit type | P99 target |
|---|---|
| Non-Rust changes (docs, YAML, shell) | < 3s |
| Rust changes only (no contracts) | < 15s |
| Rust + contract changes | < 25s |
| Worst case | < 30s |

## Installing / reinstalling hooks

```bash
scripts/setup-local-dev.sh
```

To verify hooks are installed:

```bash
ls -la .git/hooks/pre-commit .git/hooks/commit-msg .git/hooks/pre-push
```

## Bypassing hooks (emergency use only)

```bash
SKIP_HOOKS=1 git commit -m "emergency: ..."
```

All `SKIP_HOOKS=1` commits are traceable via shell history and git log.
CI will still enforce all checks regardless of local bypass.

**Do not use `--no-verify`** — it bypasses hooks silently with no audit trail.

## Conventional Commits format

```
type(scope): description (≤ 100 chars)
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`,
`build`, `ci`, `chore`, `security`, `contract`

## Monitoring and alerting

Metrics are emitted to Prometheus pushgateway at
`$PROMETHEUS_PUSHGATEWAY_URL` (default: `http://localhost:9091`).

Dashboards:
- `monitoring/grafana/dashboards/pre-commit-hooks.json` — import into Grafana

Alert rules:
- `monitoring/prometheus/alerts/pre-commit-hooks.yml`

| Alert | Condition | Severity |
|---|---|---|
| `PreCommitHookFailureSpike` | >0.1 failures/s in 30m for 5m | warning |
| `PreCommitHookTooSlow` | duration >30s for 2m | warning |
| `PreCommitHookMetricsStale` | no metrics from workstation in 48h | info |

## Deployment

Because hooks live in `scripts/hooks/` (committed) and are installed into
`.git/hooks/` (not committed), rollout works as follows:

1. Merge hook changes to `main` via normal PR.
2. Every developer pulls `main` and re-runs `scripts/setup-local-dev.sh`.
3. The CI `hook-tests` job (BATS) acts as the quality gate before merge.

No blue-green strategy is needed for local hooks — CI is the canary.

## Running hook tests locally

```bash
# Run full hook test suite
bats tests/hooks/

# Run hook tests via ci-local.sh
scripts/ci-local.sh hooks

# Lint all hook scripts
shellcheck --severity=warning scripts/hooks/*.sh
```

## Troubleshooting

**Hook passes locally but CI lint fails:**
The hook runs `rustfmt`/`clippy` only on staged files. Unstaged changes that
affect formatting will be caught by CI. Stage all related changes before committing.

**Clippy passes in hook but fails in CI:**
CI runs `--workspace --all-targets`. The hook may run only against the changed
package. Reproduce locally:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

**gitleaks false positive:**
Add the pattern to the `allowlist.regexes` block in `.gitleaks.toml` and open a PR
for review before merging.

**Hook is too slow:**
Check if the Cargo `target/` cache is warm. The first run after `cargo clean`
will be slower. If consistently slow, check which check group is the bottleneck:

```bash
SKIP_HOOKS=0 time git commit --allow-empty -m "chore: benchmark hook"
```
