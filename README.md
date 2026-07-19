# Lumina-Core

Smart contracts for blockchain-based vesting vault and token streaming infrastructure with governance, compliance, and cross-chain capabilities on Stellar Soroban.

## 🚀 Key Features
* **Defensive Governance:** Shifting control to a collaborative ecosystem with challenge periods and veto powers for beneficiaries.
* **Auto-Stake Integration:** Synchronous cross-contract staking that generates yield for beneficiaries without transferring vault assets.
* **Dead-Man's Switch (Inheritance):** Integrated inactivity timer allows primary owners to nominate backups to claim vault ownership in key-loss events.

## 🛠️ Tech Stack
* **Language/Framework:** Rust / Soroban WASM
* **Key Dependencies:** `soroban-sdk`

## 📦 Getting Started

### Prerequisites
Ensure you have the required toolchains installed:
* Rust toolchain (cargo, rustc)
* Stellar CLI / Soroban CLI

### Installation & Local Setup
```bash
# Clone the repository (if running manually)
git clone https://github.com/Lumina-etwork/Lumina-Core

# Build the smart contracts
cargo build --target wasm32-unknown-unknown --release

# Run workspace tests
cargo test
```

## 🤝 Contributing
Contributions are highly welcome. Please ensure your commits are cryptographically signed using GPG or SSH keys. For major structural changes, please open an issue first to discuss your proposal.

## Secret Rotation Service

Lumina-Core includes a deterministic secret rotation service for database credentials and API keys. The service is designed for system-wide rollout without putting plaintext secret material in process memory longer than required by the external secret manager.

### Architecture
* **Secret manager boundary:** callers keep plaintext credentials in Vault, AWS Secrets Manager, Kubernetes Secrets, or an equivalent control plane. Lumina stores only version metadata and a BLAKE2s digest for audit correlation.
* **Blue-green deployment:** every rotation creates a current version and a candidate version. The candidate enters a canary phase, then is promoted only after the overlap window has elapsed.
* **Critical-path budget:** promotion records P99 lookup latency and rejects rotations that exceed the 100 ms P99 target.
* **Availability guardrail:** canary analysis requires at least 99.90% successful requests before promotion.
* **Rollback:** failed canaries are moved back to 0% candidate traffic while retaining an auditable terminal state.

### Monitoring and alerts
Export `RotationMetrics` as Prometheus gauges/counters:

* `secret_rotation_started_total`
* `secret_rotation_promoted_total`
* `secret_rotation_rolled_back_total`
* `secret_rotation_policy_violations_total`
* `secret_rotation_active_secret_age_seconds`
* `secret_rotation_lookup_p99_latency_ms`

Alert when policy violations increase, active secret age exceeds the descriptor max age, or P99 latency is above 100 ms for five minutes.

### Runbook
1. Create a candidate secret in the external secret manager.
2. Call `plan_rotation` with the current and candidate versions.
3. Call `begin_canary` with 1-5% traffic and watch errors, authentication failures, and P99 latency.
4. Promote only after the overlap window and successful canary analysis.
5. Roll back immediately on authentication spikes, policy violations, or downstream connection pool churn.
6. Retire the previous secret from the external secret manager after all consumers have refreshed.
