# Lumina-Core

Smart contracts for blockchain-based vesting vault and token streaming infrastructure with governance, compliance, and cross-chain capabilities on Stellar Soroban.

## 🚀 Key Features
* **Defensive Governance:** Shifting control to a collaborative ecosystem with challenge periods and veto powers for beneficiaries.
* **Auto-Stake Integration:** Synchronous cross-contract staking that generates yield for beneficiaries without transferring vault assets.
* **Dead-Man's Switch (Inheritance):** Integrated inactivity timer allows primary owners to nominate backups to claim vault ownership in key-loss events.

## 🛠️ Tech Stack
* **Language/Framework:** Rust / Soroban WASM
* **Key Dependencies:** `soroban-sdk`

## 🧪 Operational Readiness
* **Staging Chaos Engineering:** See the [staging chaos engineering blueprint](docs/chaos-engineering-staging.md) for experiment guardrails, observability requirements, blue-green canary gates, and runbook expectations.

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