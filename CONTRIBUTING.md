# Contributing to Lumina Core

## Linting and Formatting

This project enforces strict code formatting and linting using `rustfmt` and `cargo clippy`. 

Before pushing any changes, ensure that your code is formatted and passes all lints. We use a pre-commit hook to automate this check.

### Setting up Pre-commit Hooks

To ensure you don't commit unformatted code or code that fails clippy lints, please install the pre-commit hooks:

```bash
# On Linux/macOS or Git Bash (Windows)
./scripts/install-hooks.sh
```

This script will copy the `scripts/pre-commit` file into `.git/hooks/pre-commit` and make it executable. The hook runs `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` on every commit.

### Manual Verification

If you prefer to run the checks manually, use the following commands:

```bash
# Auto-format your code
cargo fmt

# Run lints
cargo clippy --all-targets --all-features -- -D warnings
```
