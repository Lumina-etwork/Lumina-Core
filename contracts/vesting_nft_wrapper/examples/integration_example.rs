// Example integration of VestingNFTWrapper with VestingContract
// This demonstrates the complete workflow for OTC trading of locked allocations

use soroban_sdk::{Address, Env, String, i128, u64};
use vesting_nft_wrapper::VestingNFTWrapperClient;
use vesting_contract::VestingContractClient;

pub fn main() {
    let env = Env::default();
    
    // 1. Setup addresses
    let admin = Address::generate(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    // 2. Deploy VestingContract
    let vesting_contract_address = env.register_contract(None, VestingContract);
    let vesting_client = VestingContractClient::new(&env, &vesting_contract_address);
    
    vesting_client.initialize(&admin, &10000000i128); // 10M tokens
    vesting_client.set_token(&token_address);
    
    // 3. Deploy VestingNFTWrapper
    let nft_contract_address = env.register_contract(None, VestingNFTWrapper);
    let nft_client = VestingNFTWrapperClient::new(&env, &nft_contract_address);
    
    nft_client.initialize(
        &admin,
        &vesting_contract_address,
        &String::from_str(&env, "Vesting NFT"),
        &String::from_str(&env, "VNFT"),
    );
    
    // 4. Create a transferable vault for Investor A
    let vault_id = vesting_client.create_vault_full(
        &investor_a,
        &1000000i128, // 1M tokens
        &env.ledger().timestamp(), // Start now
        &(env.ledger().timestamp() + 31536000), // 1 year vesting
        &0i128, // No keeper fee
        &true, // Revocable
        &true, // Transferable (important for NFT wrapping)
        &0u64, // No step duration
    );
    
    println!("Created vault {} for Investor A", vault_id);
    
    // 5. Mint NFT wrapping the vault
    let token_id = nft_client.mint_vesting_nft(&vault_id, &investor_a);
    println!("Minted NFT {} wrapping vault {}", token_id, vault_id);
    
    // 6. Verify NFT ownership and metadata
    assert_eq!(nft_client.owner_of(&token_id), investor_a);
    assert_eq!(nft_client.balance_of(&investor_a), 1);
    
    let metadata = nft_client.token_metadata(&token_id);
    println!("NFT Metadata - Vault: {}, Amount: {}, Start: {}, End: {}", 
        metadata.vault_id, metadata.total_amount, metadata.start_time, metadata.end_time);
    
    // 7. Simulate OTC trade - Investor A sells to Investor B
    println!("Investor A transferring NFT to Investor B (OTC trade)...");
    nft_client.transfer(&token_id, &investor_b);
    
    // 8. Verify ownership transfer
    assert_eq!(nft_client.owner_of(&token_id), investor_b);
    assert_eq!(nft_client.balance_of(&investor_a), 0);
    assert_eq!(nft_client.balance_of(&investor_b), 1);
    
    // 9. Verify vault ownership updated in VestingContract
    let vault = vesting_client.get_vault(&vault_id);
    assert_eq!(vault.owner, investor_b);
    
    println!("OTC trade completed! Investor B now owns the vault rights");
    
    // 10. Investor B claims some tokens
    let claimable_amount = nft_client.get_claimable_amount(&token_id);
    if claimable_amount > 0 {
        let claimed = nft_client.claim_tokens(&token_id, &claimable_amount);
        println!("Investor B claimed {} tokens", claimed);
    }
    
    // 11. Check remaining claimable
    let remaining = nft_client.get_claimable_amount(&token_id);
    println!("Remaining claimable: {} tokens", remaining);
    
    println!("Integration example completed successfully!");
}

// Example of emergency lock functionality
pub fn emergency_lock_example() {
    let env = Env::default();
    
    let admin = Address::generate(&env);
    let nft_contract_address = env.register_contract(None, VestingNFTWrapper);
    let nft_client = VestingNFTWrapperClient::new(&env, &nft_contract_address);
    
    // Initialize contract
    nft_client.initialize(
        &admin,
        &Address::generate(&env),
        &String::from_str(&env, "Vesting NFT"),
        &String::from_str(&env, "VNFT"),
    );
    
    // Check initial state
    assert!(!nft_client.is_transfer_locked());
    
    // Admin locks transfers during emergency
    nft_client.set_transfer_lock(&true);
    assert!(nft_client.is_transfer_locked());
    
    // Transfers would now fail
    // nft_client.transfer(&1, &Address::generate(&env)); // Would panic
    
    // Unlock when emergency is over
    nft_client.set_transfer_lock(&false);
    assert!(!nft_client.is_transfer_locked());
    
    println!("Emergency lock functionality verified");
}

// Example of batch operations (future enhancement)
pub fn batch_operations_example() {
    println!("Batch operations example - This would be implemented in a future version");
    println!("Potential features:");
    println!("- mint_multiple_vaults(vault_ids, recipients)");
    println!("- transfer_multiple(token_ids, recipients)");
    println!("- claim_multiple(token_ids, amounts)");
}
