.PHONY: build test deploy clean

build:
	cargo build --target wasm32-unknown-unknown --release

test:
	cargo test

deploy:
	soroban contract deploy --wasm target/wasm32-unknown-unknown/release/vesting_nft_wrapper.wasm --source admin --network testnet

clean:
	cargo clean

# Integration test with vesting contract
integration-test:
	cargo test --features integration_test -- --nocapture

# Deploy both contracts for testing
deploy-full:
	$(MAKE) -C ../vesting_contracts deploy
	$(MAKE) deploy
	./scripts/setup_integration.sh

# Check contract
check:
	soroban contract inspect --wasm target/wasm32-unknown-unknown/release/vesting_nft_wrapper.wasm

# Bindings generation
bindings:
	soroban contract bindings typescript --wasm target/wasm32-unknown-unknown/release/vesting_nft_wrapper.wasm --output ./bindings/
