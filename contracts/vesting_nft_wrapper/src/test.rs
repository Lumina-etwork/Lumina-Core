use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke}, token};

#[test]
fn test_nft_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let name = String::from_str(&env, "Vesting NFT");
    let symbol = String::from_str(&env, "VNFT");
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        name.clone(),
        symbol.clone(),
    );
    
    assert_eq!(VestingNFTWrapper::name(env.clone()), name);
    assert_eq!(VestingNFTWrapper::symbol(env.clone()), symbol);
    assert_eq!(VestingNFTWrapper::total_supply(env.clone()), 0);
}

#[test]
fn test_nft_minting() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    // Mock the vault info response
    env.register_contract(
        &vesting_contract,
        crate::vesting_contract::VestingContractClient,
    );
    
    // This test would need proper mocking of the vesting contract
    // For now, we'll test the basic structure
    
    assert_eq!(VestingNFTWrapper::total_supply(env.clone()), 0);
    assert_eq!(VestingNFTWrapper::balance_of(env.clone(), beneficiary.clone()), 0);
}

#[test]
fn test_nft_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    // Mock successful transfer by setting up token ownership
    let token_id = 1u32;
    env.storage().instance().set(&DataKey::TokenOwner(token_id), &owner);
    env.storage().instance().set(&DataKey::OwnerTokens(owner.clone()), &vec![&env, token_id]);
    
    // Set up vault reference
    let vault_ref = VaultReference {
        vesting_contract: vesting_contract.clone(),
        vault_id: 1,
    };
    env.storage().instance().set(&DataKey::TokenVaultRef(token_id), &vault_ref);
    
    // Test transfer
    VestingNFTWrapper::transfer(env.clone(), token_id, new_owner.clone());
    
    // Verify ownership changed
    assert_eq!(VestingNFTWrapper::owner_of(env.clone(), token_id), new_owner);
    assert_eq!(VestingNFTWrapper::balance_of(env.clone(), owner), 0);
    assert_eq!(VestingNFTWrapper::balance_of(env.clone(), new_owner), 1);
}

#[test]
#[should_panic(expected = "Cannot transfer to self")]
fn test_transfer_to_self_fails() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let owner = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    let token_id = 1u32;
    env.storage().instance().set(&DataKey::TokenOwner(token_id), &owner);
    env.storage().instance().set(&DataKey::OwnerTokens(owner.clone()), &vec![&env, token_id]);
    
    // Try to transfer to self - should panic
    VestingNFTWrapper::transfer(env.clone(), token_id, owner.clone());
}

#[test]
#[should_panic(expected = "Transfers are locked")]
fn test_transfer_when_locked_fails() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    // Lock transfers
    VestingNFTWrapper::set_transfer_lock(env.clone(), true);
    assert!(VestingNFTWrapper::is_transfer_locked(env.clone()));
    
    let token_id = 1u32;
    env.storage().instance().set(&DataKey::TokenOwner(token_id), &owner);
    
    // Try to transfer when locked - should panic
    VestingNFTWrapper::transfer(env.clone(), token_id, new_owner);
}

#[test]
fn test_token_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    let token_id = 1u32;
    let metadata = VestingNFTMetadata {
        vault_id: 123,
        vesting_contract: vesting_contract.clone(),
        total_amount: 1000,
        start_time: 1000,
        end_time: 2000,
        created_at: 500,
        title: String::from_str(&env, "Test Vault"),
    };
    
    env.storage().instance().set(&DataKey::TokenMetadata(token_id), &metadata);
    
    let retrieved = VestingNFTWrapper::token_metadata(env.clone(), token_id);
    assert_eq!(retrieved.vault_id, 123);
    assert_eq!(retrieved.total_amount, 1000);
    assert_eq!(retrieved.title, String::from_str(&env, "Test Vault"));
}

#[test]
fn test_vault_reference() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    let token_id = 1u32;
    let vault_ref = VaultReference {
        vesting_contract: vesting_contract.clone(),
        vault_id: 456,
    };
    
    env.storage().instance().set(&DataKey::TokenVaultRef(token_id), &vault_ref);
    
    let retrieved = VestingNFTWrapper::token_vault_reference(env.clone(), token_id);
    assert_eq!(retrieved.vault_id, 456);
    assert_eq!(retrieved.vesting_contract, vesting_contract);
}

#[test]
fn test_owner_tokens_query() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let owner = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    let token_ids = vec![&env, 1u32, 2u32, 3u32];
    env.storage().instance().set(&DataKey::OwnerTokens(owner.clone()), &token_ids);
    
    let retrieved = VestingNFTWrapper::tokens_of_owner(env.clone(), owner.clone());
    assert_eq!(retrieved.len(), 3);
    assert_eq!(VestingNFTWrapper::balance_of(env.clone(), owner), 3);
}

#[test]
fn test_admin_functions() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    // Test transfer lock
    assert!(!VestingNFTWrapper::is_transfer_locked(env.clone()));
    VestingNFTWrapper::set_transfer_lock(env.clone(), true);
    assert!(VestingNFTWrapper::is_transfer_locked(env.clone()));
    VestingNFTWrapper::set_transfer_lock(env.clone(), false);
    assert!(!VestingNFTWrapper::is_transfer_locked(env.clone()));
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialization_fails() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Vesting NFT"),
        String::from_str(&env, "VNFT"),
    );
    
    // Second initialization should fail
    VestingNFTWrapper::initialize(
        env.clone(),
        admin.clone(),
        vesting_contract.clone(),
        String::from_str(&env, "Another NFT"),
        String::from_str(&env, "ANFT"),
    );
}
