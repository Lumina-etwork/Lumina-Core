# Vesting NFT Wrapper

A Soroban smart contract that wraps vesting schedule vaults into non-fungible tokens (NFTs), enabling over-the-counter (OTC) trading of locked token allocations.

## Overview

High-tier investors often want to trade their locked allocations over-the-counter (OTC). This contract implements logic that wraps a vesting schedule into a non-fungible token (NFT). If the NFT is transferred, the claim rights for the underlying locked tokens automatically transfer to the new owner's address.

## Key Features

- **NFT Minting**: Wrap existing vesting vaults into NFTs
- **Automatic Rights Transfer**: Claim rights follow NFT ownership
- **OTC Trading**: Enable secondary market for locked allocations
- **Safety Controls**: Transfer locks and admin controls
- **Metadata**: Rich metadata about underlying vesting schedules
- **Standards Compliant**: NFT standard interface implementation

## Architecture

```
Vesting Contract (Existing)
        |
        | 1. Create vault with vesting schedule
        |
        v
Vesting NFT Wrapper (This Contract)
        |
        | 2. Mint NFT that wraps the vault
        |
        v
NFT Holder
        |
        | 3. Transfer NFT to new owner
        |
        v
New NFT Holder (Gets claim rights)
```

## Contract Functions

### Initialization
- `initialize(admin, vesting_contract, name, symbol)` - Initialize the NFT wrapper

### NFT Operations
- `mint_vesting_nft(vault_id, to)` - Mint NFT wrapping a vault
- `transfer(token_id, to)` - Transfer NFT and update vault ownership
- `claim_tokens(token_id, amount)` - Claim tokens from wrapped vault

### Query Functions
- `owner_of(token_id)` - Get NFT owner
- `balance_of(owner)` - Get NFT balance for owner
- `tokens_of_owner(owner)` - Get all NFTs owned by address
- `token_metadata(token_id)` - Get NFT metadata
- `token_vault_reference(token_id)` - Get underlying vault reference
- `get_claimable_amount(token_id)` - Get claimable amount for NFT

### Admin Functions
- `set_transfer_lock(locked)` - Enable/disable transfers
- `is_transfer_locked()` - Check if transfers are locked

## Data Structures

### VestingNFTMetadata
```rust
pub struct VestingNFTMetadata {
    pub vault_id: u64,
    pub vesting_contract: Address,
    pub total_amount: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub created_at: u64,
    pub title: String,
}
```

### VaultReference
```rust
pub struct VaultReference {
    pub vesting_contract: Address,
    pub vault_id: u64,
}
```

## Security Features

1. **Transfer Lock**: Admin can lock/unlock transfers for emergency situations
2. **Authorization**: All transfers require NFT owner authorization
3. **Vault Validation**: Only transferable vaults can be wrapped
4. **Ownership Sync**: NFT ownership is always synchronized with vault ownership

## Usage Example

### 1. Initialize Contract
```rust
VestingNFTWrapper::initialize(
    env,
    admin_address,
    vesting_contract_address,
    "Vesting NFT".into(),
    "VNFT".into(),
);
```

### 2. Mint NFT from Vault
```rust
let token_id = VestingNFTWrapper::mint_vesting_nft(
    env,
    vault_id,
    investor_address,
);
```

### 3. Transfer NFT (OTC Trade)
```rust
VestingNFTWrapper::transfer(
    env,
    token_id,
    new_investor_address,
);
```

### 4. Claim Tokens
```rust
let claimed = VestingNFTWrapper::claim_tokens(
    env,
    token_id,
    claim_amount,
);
```

## Integration with Vesting Contract

The NFT wrapper integrates with the existing `VestingContract` through:

1. **Vault Information Retrieval**: Gets vault details for metadata
2. **Beneficiary Transfer**: Updates vault ownership when NFT transfers
3. **Token Claiming**: Claims tokens on behalf of NFT owner
4. **Claimable Amount**: Queries vested amount for wrapped vault

## Gas Costs

| Operation | Estimated Cost |
|-----------|----------------|
| Initialize | ~0.02 XLM |
| Mint NFT | ~0.03 XLM |
| Transfer NFT | ~0.025 XLM |
| Claim Tokens | ~0.02 XLM |
| Query Metadata | ~0.005 XLM |

## Events

- `nft_minted` - Emitted when NFT is minted
- `nft_transferred` - Emitted when NFT is transferred
- `claim_rights_transferred` - Emitted when claim rights change

## Error Conditions

- `"Already initialized"` - Contract already initialized
- `"Vault is not transferable"` - Vault cannot be wrapped
- `"Cannot transfer to self"` - Self-transfer not allowed
- `"Transfers are locked"` - Transfers are currently disabled
- `"Token not found"` - NFT token ID doesn't exist
- `"Not initialized"` - Contract not initialized

## Testing

Run tests with:
```bash
cargo test --package vesting_nft_wrapper
```

## Deployment

1. Deploy the `VestingContract` first
2. Deploy this `VestingNFTWrapper` contract
3. Initialize with admin address and vesting contract address
4. Set token address in vesting contract
5. Create transferable vaults in vesting contract
6. Mint NFTs for vaults

## Future Enhancements

- **Batch Operations**: Mint/transfer multiple NFTs in single transaction
- **Marketplace Integration**: Built-in marketplace functionality
- **Fractional Ownership**: Enable fractional NFT ownership
- **Cross-chain Support**: Bridge to other chains
- **Advanced Vesting**: Support for complex vesting schedules

## License

This contract is part of the vesting system and follows the same licensing terms.
