#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, vec, Address, Env, IntoVal, Map, Symbol, Val, Vec, String,
};

// NFT Token Standard Constants
const NFT_NAME: Symbol = symbol_short!("NFT_NAME");
const NFT_SYMBOL: Symbol = symbol_short!("NFT_SYM");
const NFT_TOKEN_ID: Symbol = symbol_short!("TOKEN_ID");
const NFT_OWNER: Symbol = symbol_short!("OWNER");
const NFT_METADATA: Symbol = symbol_short!("META");
const NFT_VAULT_REF: Symbol = symbol_short!("VAULT");

// Contract Storage Keys
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // NFT Standard
    Name,
    Symbol,
    TokenCount,
    TokenOwner(u32),  // token_id -> owner
    OwnerTokens(Address),  // owner -> Vec<token_id>
    TokenMetadata(u32),  // token_id -> metadata
    TokenVaultRef(u32),  // token_id -> (vesting_contract_address, vault_id)
    
    // Contract Admin
    AdminAddress,
    VestingContractAddress,
    
    // Transfer Safety
    TransferLock,
}

#[contracttype]
#[derive(Clone)]
pub struct VestingNFTMetadata {
    pub vault_id: u64,
    pub vesting_contract: Address,
    pub total_amount: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub created_at: u64,
    pub title: String,
}

#[contracttype]
#[derive(Clone)]
pub struct VaultReference {
    pub vesting_contract: Address,
    pub vault_id: u64,
}

#[contracttype]
pub struct NFTMinted {
    pub token_id: u32,
    pub owner: Address,
    pub vault_reference: VaultReference,
}

#[contracttype]
pub struct NFTTransferred {
    pub token_id: u32,
    pub from: Address,
    pub to: Address,
    pub vault_reference: VaultReference,
}

#[contracttype]
pub struct ClaimRightsTransferred {
    pub token_id: u32,
    pub new_owner: Address,
    pub vault_reference: VaultReference,
}

#[contract]
pub struct VestingNFTWrapper;

#[contractimpl]
impl VestingNFTWrapper {
    // Initialize the NFT wrapper contract
    pub fn initialize(env: Env, admin: Address, vesting_contract: Address, name: String, symbol: String) {
        if env.storage().instance().has(&DataKey::AdminAddress) {
            panic!("Already initialized");
        }
        
        env.storage().instance().set(&DataKey::AdminAddress, &admin);
        env.storage().instance().set(&DataKey::VestingContractAddress, &vesting_contract);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::TokenCount, &0u32);
        env.storage().instance().set(&DataKey::TransferLock, &false);
    }

    // Mint an NFT that wraps a vesting vault
    pub fn mint_vesting_nft(env: Env, vault_id: u64, to: Address) -> u32 {
        Self::require_admin(&env);
        
        // Get vault information from vesting contract
        let vesting_contract = Self::get_vesting_contract(&env);
        let vault_info = Self::get_vault_info(&env, &vesting_contract, vault_id);
        
        // Verify vault exists and is transferable
        if !vault_info.is_transferable {
            panic!("Vault is not transferable");
        }
        
        // Generate new token ID
        let token_id = Self::increment_token_count(&env);
        
        // Create metadata
        let metadata = VestingNFTMetadata {
            vault_id,
            vesting_contract: vesting_contract.clone(),
            total_amount: vault_info.total_amount,
            start_time: vault_info.start_time,
            end_time: vault_info.end_time,
            created_at: env.ledger().timestamp(),
            title: vault_info.title.clone(),
        };
        
        // Create vault reference
        let vault_ref = VaultReference {
            vesting_contract: vesting_contract.clone(),
            vault_id,
        };
        
        // Store NFT data
        env.storage().instance().set(&DataKey::TokenOwner(token_id), &to);
        env.storage().instance().set(&DataKey::TokenMetadata(token_id), &metadata);
        env.storage().instance().set(&DataKey::TokenVaultRef(token_id), &vault_ref);
        
        // Update owner's token list
        Self::add_token_to_owner(&env, &to, token_id);
        
        // Transfer vault ownership to NFT contract (it will manage claims)
        Self::transfer_vault_ownership_to_nft(&env, &vesting_contract, vault_id);
        
        // Emit mint event
        let mint_event = NFTMinted {
            token_id,
            owner: to.clone(),
            vault_reference: vault_ref,
        };
        env.events().publish((Symbol::new(&env, "nft_minted"), token_id), mint_event);
        
        token_id
    }

    // Transfer NFT and update vault ownership
    pub fn transfer(env: Env, token_id: u32, to: Address) {
        Self::require_not_locked(&env);
        
        let from = Self::get_token_owner(&env, token_id);
        from.require_auth();
        
        // Cannot transfer to yourself
        if from == to {
            panic!("Cannot transfer to self");
        }
        
        // Get vault reference
        let vault_ref = Self::get_vault_reference(&env, token_id);
        
        // Update NFT ownership
        env.storage().instance().set(&DataKey::TokenOwner(token_id), &to);
        
        // Update owner token lists
        Self::remove_token_from_owner(&env, &from, token_id);
        Self::add_token_to_owner(&env, &to, token_id);
        
        // Update vault ownership in vesting contract
        Self::update_vault_beneficiary(&env, &vault_ref.vesting_contract, vault_ref.vault_id, &to);
        
        // Emit transfer event
        let transfer_event = NFTTransferred {
            token_id,
            from: from.clone(),
            to: to.clone(),
            vault_reference: vault_ref.clone(),
        };
        env.events().publish((Symbol::new(&env, "nft_transferred"), token_id), transfer_event);
        
        // Emit claim rights transfer event
        let claim_event = ClaimRightsTransferred {
            token_id,
            new_owner: to.clone(),
            vault_reference: vault_ref,
        };
        env.events().publish((Symbol::new(&env, "claim_rights_transferred"), token_id), claim_event);
    }

    // Claim tokens from a vault owned by NFT holder
    pub fn claim_tokens(env: Env, token_id: u32, claim_amount: i128) -> i128 {
        let owner = Self::get_token_owner(&env, token_id);
        owner.require_auth();
        
        let vault_ref = Self::get_vault_reference(&env, token_id);
        
        // Call claim on the vesting contract
        let vesting_contract = Self::get_vesting_contract(&env);
        let client = crate::vesting_contract::VestingContractClient::new(&env, &vault_ref.vesting_contract);
        client.claim_tokens(&vault_ref.vault_id, &claim_amount)
    }

    // Get claimable amount for an NFT
    pub fn get_claimable_amount(env: Env, token_id: u32) -> i128 {
        let vault_ref = Self::get_vault_reference(&env, token_id);
        let vesting_contract = Self::get_vesting_contract(&env);
        let client = crate::vesting_contract::VestingContractClient::new(&env, &vault_ref.vesting_contract);
        client.get_claimable_amount(&vault_ref.vault_id)
    }

    // Query functions
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        Self::get_token_owner(&env, token_id)
    }

    pub fn balance_of(env: Env, owner: Address) -> u32 {
        let tokens = Self::get_owner_tokens(&env, &owner);
        tokens.len() as u32
    }

    pub fn tokens_of_owner(env: Env, owner: Address) -> Vec<u32> {
        Self::get_owner_tokens(&env, &owner)
    }

    pub fn token_metadata(env: Env, token_id: u32) -> VestingNFTMetadata {
        env.storage().instance()
            .get(&DataKey::TokenMetadata(token_id))
            .expect("Token not found")
    }

    pub fn token_vault_reference(env: Env, token_id: u32) -> VaultReference {
        Self::get_vault_reference(&env, token_id)
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).expect("Not initialized")
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).expect("Not initialized")
    }

    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TokenCount).unwrap_or(0)
    }

    // Admin functions
    pub fn set_transfer_lock(env: Env, locked: bool) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::TransferLock, &locked);
    }

    pub fn is_transfer_locked(env: Env) -> bool {
        env.storage().instance().get(&DataKey::TransferLock).unwrap_or(false)
    }

    // --- Internal Helper Functions ---

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::AdminAddress).expect("Admin not set");
        admin.require_auth();
    }

    fn require_not_locked(env: &Env) {
        if env.storage().instance().get(&DataKey::TransferLock).unwrap_or(false) {
            panic!("Transfers are locked");
        }
    }

    fn get_vesting_contract(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::VestingContractAddress).expect("Vesting contract not set")
    }

    fn increment_token_count(env: &Env) -> u32 {
        let count: u32 = env.storage().instance().get(&DataKey::TokenCount).unwrap_or(0);
        let new_count = count + 1;
        env.storage().instance().set(&DataKey::TokenCount, &new_count);
        new_count
    }

    fn get_token_owner(env: &Env, token_id: u32) -> Address {
        env.storage().instance()
            .get(&DataKey::TokenOwner(token_id))
            .expect("Token not found")
    }

    fn get_vault_reference(env: &Env, token_id: u32) -> VaultReference {
        env.storage().instance()
            .get(&DataKey::TokenVaultRef(token_id))
            .expect("Token not found")
    }

    fn get_owner_tokens(env: &Env, owner: &Address) -> Vec<u32> {
        env.storage().instance()
            .get(&DataKey::OwnerTokens(owner.clone()))
            .unwrap_or(Vec::new(env))
    }

    fn add_token_to_owner(env: &Env, owner: &Address, token_id: u32) {
        let mut tokens = Self::get_owner_tokens(env, owner);
        tokens.push_back(token_id);
        env.storage().instance().set(&DataKey::OwnerTokens(owner.clone()), &tokens);
    }

    fn remove_token_from_owner(env: &Env, owner: &Address, token_id: u32) {
        let mut tokens = Self::get_owner_tokens(env, owner);
        let mut new_tokens = Vec::new(env);
        
        for token in tokens.iter() {
            if token != token_id {
                new_tokens.push_back(token);
            }
        }
        
        env.storage().instance().set(&DataKey::OwnerTokens(owner.clone()), &new_tokens);
    }

    fn get_vault_info(env: &Env, vesting_contract: &Address, vault_id: u64) -> vesting_contract::VaultInfo {
        let client = VestingContractClient::new(env, vesting_contract);
        client.get_vault_info(vault_id)
    }

    fn transfer_vault_ownership_to_nft(env: &Env, vesting_contract: &Address, vault_id: u64) {
        let client = VestingContractClient::new(env, vesting_contract);
        let nft_contract = env.current_contract_address();
        client.transfer_beneficiary(&vault_id, &nft_contract);
    }

    fn update_vault_beneficiary(env: &Env, vesting_contract: &Address, vault_id: u64, new_owner: &Address) {
        let client = VestingContractClient::new(env, vesting_contract);
        client.transfer_beneficiary(&vault_id, new_owner);
    }
}

// Vesting contract client that interfaces with the existing VestingContract
mod vesting_contract {
    use soroban_sdk::{Address, Env, i128, String, Vec};
    
    // Vault struct matching the VestingContract interface
    #[derive(Clone)]
    pub struct Vault {
        pub total_amount: i128,
        pub released_amount: i128,
        pub keeper_fee: i128,
        pub staked_amount: i128,
        pub owner: Address,
        pub delegate: Option<Address>,
        pub title: String,
        pub start_time: u64,
        pub end_time: u64,
        pub creation_time: u64,
        pub step_duration: u64,
        pub is_initialized: bool,
        pub is_irrevocable: bool,
        pub is_transferable: bool,
        pub is_frozen: bool,
    }
    
    // Simplified vault info for NFT metadata
    pub struct VaultInfo {
        pub total_amount: i128,
        pub start_time: u64,
        pub end_time: u64,
        pub title: String,
        pub is_transferable: bool,
    }
    
    pub struct VestingContractClient<'a> {
        env: &'a Env,
        address: &'a Address,
    }
    
    #[contractclient(crate::VestingContractClient::client)]
    trait VestingContractInterface {
        fn claim_tokens(env: &Env, vault_id: u64, claim_amount: i128) -> i128;
        fn get_claimable_amount(env: &Env, vault_id: u64) -> i128;
        fn get_vault(env: &Env, vault_id: u64) -> Vault;
        fn transfer_beneficiary(env: &Env, vault_id: u64, new_address: &Address);
    }
    
    impl<'a> VestingContractClient<'a> {
        pub fn new(env: &'a Env, address: &'a Address) -> Self {
            Self { env, address }
        }
        
        pub fn claim_tokens(&self, vault_id: &u64, amount: &i128) -> i128 {
            self.env.invoke_contract(
                self.address,
                &VestingContractInterface::claim_tokens,
                (vault_id, amount),
            )
        }
        
        pub fn get_claimable_amount(&self, vault_id: &u64) -> i128 {
            self.env.invoke_contract(
                self.address,
                &VestingContractInterface::get_claimable_amount,
                vault_id,
            )
        }
        
        pub fn get_vault(&self, vault_id: &u64) -> Vault {
            self.env.invoke_contract(
                self.address,
                &VestingContractInterface::get_vault,
                vault_id,
            )
        }
        
        pub fn transfer_beneficiary(&self, vault_id: &u64, new_address: &Address) {
            self.env.invoke_contract(
                self.address,
                &VestingContractInterface::transfer_beneficiary,
                (vault_id, new_address),
            );
        }
        
        pub fn get_vault_info(&self, vault_id: &u64) -> VaultInfo {
            let vault = self.get_vault(vault_id);
            VaultInfo {
                total_amount: vault.total_amount,
                start_time: vault.start_time,
                end_time: vault.end_time,
                title: vault.title,
                is_transferable: vault.is_transferable,
            }
        }
    }
}

#[cfg(test)]
mod test;
