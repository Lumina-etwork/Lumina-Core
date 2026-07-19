# Coverage Threshold Enforcement Runbook

## Architecture

The CI coverage gate runs in GitHub Actions for every pull request and for pushes
to `main` or `master`. The workflow installs `cargo-llvm-cov`, generates a
workspace coverage summary, and calls `scripts/check_coverage_threshold.sh` to
enforce the configured line-coverage percentage.

## Threshold policy

- Default threshold: `80%` line coverage.
- Override mechanism: change the `COVERAGE_THRESHOLD` environment variable in
  `.github/workflows/coverage.yml`.
- Gate behavior: CI fails fast when the parsed `TOTAL` line coverage is below
  the configured threshold, preventing merges that reduce test coverage below
  the minimum accepted level.

## Monitoring and alerting

- GitHub branch protection should require the `Rust coverage threshold` job.
- Failed checks are the primary alert for maintainers and should be treated as a
  release-blocking quality signal.
- Coverage summaries are uploaded as workflow artifacts for auditability and
  trend review.

## Deployment strategy

Roll out with branch protection in two phases:

1. **Canary:** Enable the workflow without making it required for a small set of
   pull requests. Review failures and adjust excluded code or test fixtures only
   when there is a documented reason.
2. **Enforcement:** Mark the coverage job as required in branch protection after
   the canary period is green. This is the blue-green cutover from advisory to
   enforced coverage checks.

## Troubleshooting

1. Open the failed `Rust coverage threshold` job.
2. Download the `rust-coverage-summary` artifact.
3. Compare the `TOTAL` line coverage with `COVERAGE_THRESHOLD`.
4. Add focused tests for uncovered behavior, then rerun CI.
5. If the parser fails, run `scripts/test_coverage_threshold.sh` locally to
   verify the threshold checker itself.

## Security review notes

The threshold checker is a local shell script that reads only the generated
coverage summary and performs numeric comparison. It does not evaluate dynamic
code or transmit coverage data outside GitHub Actions artifacts.
