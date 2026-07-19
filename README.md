# Lumina-Core

[![CI](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/ci.yml/badge.svg)](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/ci.yml)
[![Contracts WASM](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/contracts.yml/badge.svg)](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/contracts.yml)
[![Security Audit](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/security.yml/badge.svg)](https://github.com/Lumina-etwork/Lumina-Core/actions/workflows/security.yml)

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
cd Lumina-Core

# Validate prerequisites without changing your machine
scripts/setup-local-dev.sh --check

# Bootstrap the local toolchain, build contracts, and run tests
scripts/setup-local-dev.sh
```

The onboarding script verifies Rust, rustup, and the WASM target required for
Soroban contracts. It also detects the Stellar/Soroban CLI for deployment
workflows and supports `--dry-run`, `--skip-build`, and `--skip-tests` for
incremental setup.

## 🤝 Contributing
Contributions are highly welcome. Please ensure your commits are cryptographically signed using GPG or SSH keys. For major structural changes, please open an issue first to discuss your proposal.
