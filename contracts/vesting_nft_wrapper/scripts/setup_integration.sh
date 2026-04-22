#!/bin/bash

# Setup script for integrating VestingNFTWrapper with VestingContract
# This script demonstrates the deployment and setup process

set -e

echo "Setting up Vesting NFT Wrapper integration..."

# Configuration
NETWORK="testnet"
ADMIN_ADDRESS="G..."
VESTING_CONTRACT_ID="CD6OGC46OFCV52IJQKEDVKLX5ASA3ZMSTHAAZQIPDSJV6VZ3KUJDEP4D"
TOKEN_ADDRESS="TOKEN_ADDRESS_HERE"

echo "1. Deploying Vesting NFT Wrapper contract..."
NFT_CONTRACT_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/vesting_nft_wrapper.wasm \
    --source $ADMIN_ADDRESS \
    --network $NETWORK)

echo "NFT Contract deployed: $NFT_CONTRACT_ID"

echo "2. Initializing NFT Wrapper contract..."
soroban contract invoke \
    --id $NFT_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    initialize \
    --admin $ADMIN_ADDRESS \
    --vesting_contract $VESTING_CONTRACT_ID \
    --name "Vesting NFT" \
    --symbol "VNFT"

echo "3. Setting token address in Vesting Contract..."
soroban contract invoke \
    --id $VESTING_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    set_token \
    --token $TOKEN_ADDRESS

echo "4. Creating a sample transferable vault..."
soroban contract invoke \
    --id $VESTING_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    create_vault_full \
    --owner $ADMIN_ADDRESS \
    --amount 1000000 \
    --start_time $(date +%s) \
    --end_time $(($(date +%s) + 31536000)) \
    --keeper_fee 0 \
    --is_revocable true \
    --is_transferable true \
    --step_duration 0

echo "5. Minting NFT for the vault..."
soroban contract invoke \
    --id $NFT_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    mint_vesting_nft \
    --vault_id 1 \
    --to $ADMIN_ADDRESS

echo "6. Checking NFT ownership..."
NFT_OWNER=$(soroban contract invoke \
    --id $NFT_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    owner_of \
    --token_id 1)

echo "NFT Owner: $NFT_OWNER"

echo "7. Getting NFT metadata..."
soroban contract invoke \
    --id $NFT_CONTRACT_ID \
    --source $ADMIN_ADDRESS \
    --network $NETWORK \
    -- \
    token_metadata \
    --token_id 1

echo "Integration setup complete!"
echo ""
echo "Next steps:"
echo "1. Transfer NFT to test OTC trading"
echo "2. Test claim functionality"
echo "3. Verify vault ownership updates"

echo ""
echo "Example transfer command:"
echo "soroban contract invoke --id $NFT_CONTRACT_ID --source $ADMIN_ADDRESS --network $NETWORK -- transfer --token_id 1 --to NEW_OWNER_ADDRESS"
