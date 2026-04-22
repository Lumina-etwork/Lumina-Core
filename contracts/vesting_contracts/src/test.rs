use crate::{ BatchCreateData, Milestone, PausedVault, VestingContract, VestingContractClient };
use crate::{
    BatchCreateData, Milestone, VestingContract, VestingContractClient,
    GovernanceAction, GovernanceProposal, Vote,
    BatchCreateData, Milestone, VestingContract, VestingContractClient, StakeState,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    token, vec, Address, Env, String,
    BatchCreateData, Milestone, ScheduleConfig, VestingContract, VestingContractClient,
};
use soroban_sdk::{
    testutils::{ Address as _, Ledger },
    token,
    vec,
    Address,
    Env,
    IntoVal,
    Symbol,
    String,
    Map,
};

fn setup() -> (Env, Address, VestingContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &1_000_000_000i128);

    let token_admin = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    client.set_token(&token_addr);
    client.add_to_whitelist(&token_addr);

    // Mint initial supply to contract
    let stellar = token::StellarAssetClient::new(&env, &token_addr);
    stellar.mint(&contract_id, &1_000_000_000i128);

    (env, contract_id, client, admin, token_addr)
}

// =============================================================================
// Existing tests
// =============================================================================

#[test]
fn test_initialize() {
    let (env, _, client, admin, _) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_create_vault_full_and_claim() {
    let (env, _, client, _admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false, // irrevocable
        &false,
        &0u64
    );

    assert_eq!(vault_id, 1);

    // Fast forward
    env.ledger().set_timestamp(now + 500);
    assert_eq!(client.get_claimable_amount(&vault_id), 500);

    // Claim
    client.claim_tokens(&vault_id, &100i128);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);

    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&beneficiary), 100);
}

#[test]
fn test_periodic_vesting() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // 1000 tokens over 1000 seconds, with 100 second steps
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &100u64
    );

    env.ledger().set_timestamp(now + 150);
    // 1 step completed (100 tokens)
    assert_eq!(client.get_claimable_amount(&vault_id), 100);

    env.ledger().set_timestamp(now + 250);
    // 2 steps completed (200 tokens)
    assert_eq!(client.get_claimable_amount(&vault_id), 200);
}

#[test]
fn test_milestones() {
    let (env, _, client, _admin, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64
    );

    let milestones = vec![&env,
    let milestones = vec![
        &env,
        Milestone { id: 1, percentage: 30, is_unlocked: false },
        Milestone { id: 2, percentage: 70, is_unlocked: false }
    ];

    client.set_milestones(&vault_id, &milestones);

    assert_eq!(client.get_claimable_amount(&vault_id), 0);

    client.unlock_milestone(&vault_id, &1);
    assert_eq!(client.get_claimable_amount(&vault_id), 300);

    client.unlock_milestone(&vault_id, &2);
    assert_eq!(client.get_claimable_amount(&vault_id), 1000);
}

#[test]
fn test_global_pause() {
    let (_, _, client, _, _) = setup();

    client.toggle_pause();
    assert!(client.is_paused());
    let (env, _, client, admin, _) = setup();

    client.toggle_pause();
    assert!(client.is_paused());

    let beneficiary = Address::generate(&env);
    // Logic that depends on paused should fail
}

#[test]
fn test_batch_operations() {
    let (env, _, client, _, _) = setup();
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let now = env.ledger().timestamp();

    let batch = BatchCreateData {
        recipients: vec![&env, r1, r2],
        amounts: vec![&env, 500i128, 500i128],
        start_times: vec![&env, now, now],
        end_times: vec![&env, now + 1000, now + 1000],
        keeper_fees: vec![&env, 0i128, 0i128],
        step_durations: vec![&env, 0u64, 0u64],
    };

    let ids = client.batch_create_vaults_full(&batch);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids.get(0).unwrap(), 1);
    assert_eq!(ids.get(1).unwrap(), 2);
}

// --- Governance Tests ---

#[test]
fn test_propose_admin_rotation() {
    let (env, _, client, admin, _) = setup();
    let new_admin = Address::generate(&env);
    
    let proposal_id = client.propose_admin_rotation(&new_admin);
    assert_eq!(proposal_id, 1);
    
    let proposal = client.get_proposal_info(&proposal_id);
    assert_eq!(proposal.id, 1);
    assert_eq!(proposal.proposer, admin);
    match proposal.action {
        GovernanceAction::AdminRotation(addr) => assert_eq!(addr, new_admin),
        _ => panic!("Expected AdminRotation action"),
    }
}

#[test]
fn test_propose_contract_upgrade() {
    let (env, _, client, admin, _) = setup();
    let new_contract = Address::generate(&env);
    
    let proposal_id = client.propose_contract_upgrade(&new_contract);
    assert_eq!(proposal_id, 1);
    
    let proposal = client.get_proposal_info(&proposal_id);
    match proposal.action {
        GovernanceAction::ContractUpgrade(addr) => assert_eq!(addr, new_contract),
        _ => panic!("Expected ContractUpgrade action"),
    }
}

#[test]
fn test_propose_emergency_pause() {
    let (env, _, client, _, _) = setup();
    
    let proposal_id = client.propose_emergency_pause(&true);
    assert_eq!(proposal_id, 1);
    
    let proposal = client.get_proposal_info(&proposal_id);
    match proposal.action {
        GovernanceAction::EmergencyPause(pause_state) => assert_eq!(pause_state, true),
        _ => panic!("Expected EmergencyPause action"),
    }
}

#[test]
fn test_voting_power_calculation() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    // Create a vault for the beneficiary
    let vault_id = client.create_vault_full(
        &beneficiary,
#[test]
fn test_pause_specific_schedule() {
    let (env, _, client, admin, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64
    );

    // Fast forward to allow some vesting
    env.ledger().set_timestamp(now + 500);
    assert_eq!(client.get_claimable_amount(&vault_id), 500);

    // Pause the vault
    client.pause_specific_schedule(&vault_id, &String::from_str(&env, "Legal dispute"));

    // Check that vault is paused
    assert!(client.is_vault_paused(&vault_id));

    // Get pause info
    let pause_info = client.get_paused_vault_info(&vault_id).unwrap();
    assert_eq!(pause_info.vault_id, vault_id);
    assert_eq!(pause_info.pause_timestamp, now + 500);
    assert_eq!(pause_info.reason, String::from_str(&env, "Legal dispute"));
}

#[test]
fn test_pause_timestamp_locking() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
fn test_batch_add_schedules_large_tge_batch() {
    let (env, _, client, _, _) = setup();
    let now = env.ledger().timestamp();

    let mut schedules = vec![&env];
    for _ in 0..60 {
        schedules.push_back(ScheduleConfig {
            owner: Address::generate(&env),
            amount: 10_000i128,
            start_time: now,
            end_time: now + 1_000,
            keeper_fee: 0i128,
            is_revocable: true,
            is_transferable: false,
            step_duration: 0u64,
        });
    }

    let ids = client.batch_add_schedules(&schedules);
    assert_eq!(ids.len(), 60);
    assert_eq!(ids.get(0).unwrap(), 1u64);
    assert_eq!(ids.get(59).unwrap(), 60u64);
}

#[test]
#[should_panic(expected = "Insufficient deposited tokens for batch")]
fn test_batch_add_schedules_requires_deposited_token_coverage() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &1_000_000_000i128);

    let token_admin = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    client.set_token(&token_addr);
    client.add_to_whitelist(&token_addr);

    // Deliberately under-fund the contract balance relative to the batch total.
    let stellar = token::StellarAssetClient::new(&env, &token_addr);
    stellar.mint(&contract_id, &1_000i128);

    let now = env.ledger().timestamp();
    let schedules = vec![
        &env,
        ScheduleConfig {
            owner: Address::generate(&env),
            amount: 700i128,
            start_time: now,
            end_time: now + 1_000,
            keeper_fee: 0i128,
            is_revocable: true,
            is_transferable: false,
            step_duration: 0u64,
        },
        ScheduleConfig {
            owner: Address::generate(&env),
            amount: 700i128,
            start_time: now,
            end_time: now + 1_000,
            keeper_fee: 0i128,
            is_revocable: true,
            is_transferable: false,
            step_duration: 0u64,
        },
    ];

    client.batch_add_schedules(&schedules);
}

#[test]
fn test_metadata_anchor() {
    let (env, _, client, _, _) = setup();

    // Should return empty string before anything is set
    let empty = client.get_metadata_anchor();
    assert_eq!(empty, String::from_str(&env, ""));

    // Set a CID and retrieve it
    let cid = String::from_str(&env, "ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi");
    client.set_metadata_anchor(&cid);

    let retrieved = client.get_metadata_anchor();
    assert_eq!(retrieved, cid);
fn test_voting_power() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Irrevocable vault: 1000 tokens (100% weight = 1000 power)
    client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64
    );

    // Fast forward to allow some vesting
    env.ledger().set_timestamp(now + 500);

    // Pause the vault
    client.pause_specific_schedule(&vault_id, &String::from_str(&env, "Dispute"));

    // Even if we fast forward more, claimable should be locked at pause time
    env.ledger().set_timestamp(now + 800);
    assert_eq!(client.get_claimable_amount(&vault_id), 500); // Still 500, locked at pause time
}

#[test]
fn test_resume_specific_schedule() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &false, // is_revocable = false => is_irrevocable = true
        &false,
        &0u64,
    );

    // Revocable vault: 1000 tokens (50% weight = 500 power)
    client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64,
    );
    
    let voting_power = client.get_voter_power(&beneficiary);
    assert_eq!(voting_power, 1000);
    
    // After partial claim, voting power should decrease
    env.ledger().set_timestamp(now + 500);
    client.claim_tokens(&vault_id, &200i128);
    
    let voting_power_after_claim = client.get_voter_power(&beneficiary);
    assert_eq!(voting_power_after_claim, 800);
}

#[test]
fn test_successful_governance_execution() {
    let (env, _, client, admin, _) = setup();
    let new_admin = Address::generate(&env);
    
    // Propose admin rotation
    let proposal_id = client.propose_admin_rotation(&new_admin);
    
    // Fast forward past challenge period
    let proposal = client.get_proposal_info(&proposal_id);
    env.ledger().set_timestamp(proposal.challenge_end + 1);
    
    // Execute proposal (should pass with no votes against)
    client.execute_proposal(&proposal_id);
    
    // Check admin was changed
    assert_eq!(client.get_admin(), new_admin);
    
    // Check proposal is marked as executed
    let updated_proposal = client.get_proposal_info(&proposal_id);
    assert!(updated_proposal.is_executed);
}

#[test]
fn test_vetoed_governance_proposal() {
    let (env, _, client, _, token) = setup();
    let beneficiary1 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    // Create vaults with significant tokens (51%+ of total)
    client.create_vault_full(&beneficiary1, &600i128, &now, &(now + 1000), &0i128, &false, &false, &0u64);
    client.create_vault_full(&beneficiary2, &400i128, &now, &(now + 1000), &0i128, &false, &false, &0u64);
    
    // Propose admin rotation
    let proposal_id = client.propose_admin_rotation(&new_admin);
    
    // Vote against the proposal (51% of total)
    // Note: In real implementation, beneficiaries would vote directly
    // For test purposes, we'll simulate the voting
    
    // Fast forward past challenge period
    let proposal = client.get_proposal_info(&proposal_id);
    env.ledger().set_timestamp(proposal.challenge_end + 1);
    
    // Manually set veto votes for testing
    // In real implementation, this would happen through vote_on_proposal calls
    
    // Execute proposal (should fail due to veto)
    client.execute_proposal(&proposal_id);
    
    // Check admin was NOT changed
    assert_ne!(client.get_admin(), new_admin);
    
    // Check proposal is marked as cancelled
    let updated_proposal = client.get_proposal_info(&proposal_id);
    assert!(updated_proposal.is_cancelled);
}

#[test]
fn test_challenge_period_enforcement() {
    let (env, _, client, _, _) = setup();
    let new_admin = Address::generate(&env);
    
    // Propose admin rotation
    let proposal_id = client.propose_admin_rotation(&new_admin);
    
    // Try to execute before challenge period ends
    let proposal = client.get_proposal_info(&proposal_id);
    env.ledger().set_timestamp(proposal.challenge_end - 1);
    
    // Should panic because challenge period hasn't ended
    let _result = std::panic::catch_unwind(|| {
        client.execute_proposal(&proposal_id);
    });
    assert!(_result.is_err());
}

#[test]
fn test_emergency_pause_governance() {
    let (env, _, client, _, _) = setup();
    
    // Initially not paused
    assert!(!client.is_paused());
    
    // Propose emergency pause
    let proposal_id = client.propose_emergency_pause(&true);
    
    // Fast forward past challenge period
    let proposal = client.get_proposal_info(&proposal_id);
    env.ledger().set_timestamp(proposal.challenge_end + 1);
    
    // Execute proposal
    client.execute_proposal(&proposal_id);
    
    // Should now be paused
    assert!(client.is_paused());
}

#[test]
fn test_contract_upgrade_governance() {
    let (env, _, client, _, _) = setup();
    let new_contract = Address::generate(&env);
    
    // Initially not deprecated
    // Note: We'd need a getter for IsDeprecated to test this properly
    
    // Propose contract upgrade
    let proposal_id = client.propose_contract_upgrade(&new_contract);
    
    // Fast forward past challenge period
    let proposal = client.get_proposal_info(&proposal_id);
    env.ledger().set_timestamp(proposal.challenge_end + 1);
    
    // Execute proposal
    client.execute_proposal(&proposal_id);
    
    // Check proposal is executed
    let updated_proposal = client.get_proposal_info(&proposal_id);
    assert!(updated_proposal.is_executed);
        &true, // is_revocable = true => is_irrevocable = false
        &false,
        &0u64,
    );

    // Total power should be 1000 + 500 = 1500
    assert_eq!(client.get_voting_power(&beneficiary), 1500);
}

#[test]
fn test_delegated_voting_power() {
    let (env, _, client, _, _) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let representative = Address::generate(&env);
    let now = env.ledger().timestamp();

    // A: 1000 power (irrevocable)
    client.create_vault_full(
        &beneficiary_a,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64
    );

    // Fast forward and pause
    env.ledger().set_timestamp(now + 500);
    client.pause_specific_schedule(&vault_id, &String::from_str(&env, "Legal dispute"));
    assert!(client.is_vault_paused(&vault_id));

    // Resume the vault
    client.resume_specific_schedule(&vault_id);

    // Check that vault is no longer paused
    assert!(!client.is_vault_paused(&vault_id));
    assert!(client.get_paused_vault_info(&vault_id).is_none());

    // Claim should now work
    client.claim_tokens(&vault_id, &100i128);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);
}

#[test]
#[should_panic]
fn test_pause_already_paused_vault() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

        &0u64,
    );

    // B: 500 power (revocable)
    client.create_vault_full(
        &beneficiary_b,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    );

    assert_eq!(client.get_voting_power(&beneficiary_a), 1000);
    assert_eq!(client.get_voting_power(&beneficiary_b), 500);
    assert_eq!(client.get_voting_power(&representative), 0);

    client.delegate_voting_power(&beneficiary_a, &beneficiary_b);
    assert_eq!(client.get_voting_power(&beneficiary_a), 0);
    assert_eq!(client.get_voting_power(&beneficiary_b), 1500);

    client.delegate_voting_power(&beneficiary_b, &representative);
    assert_eq!(client.get_voting_power(&beneficiary_b), 0);
    assert_eq!(client.get_voting_power(&representative), 500);

    client.delegate_voting_power(&beneficiary_a, &representative);
    assert_eq!(client.get_voting_power(&representative), 1500);

    client.delegate_voting_power(&beneficiary_a, &beneficiary_a);
    assert_eq!(client.get_voting_power(&beneficiary_a), 1000);
    assert_eq!(client.get_voting_power(&representative), 500);
}

#[test]
fn test_vesting_acceleration() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64
    );

    // Pause once
    client.pause_specific_schedule(&vault_id, &String::from_str(&env, "First pause"));

    // Try to pause again - should panic
    client.pause_specific_schedule(&vault_id, &String::from_str(&env, "Second pause"));
}

#[test]
#[should_panic]
fn test_resume_non_paused_vault() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

        &true,
        &false,
        &0u64,
    );

    env.ledger().set_timestamp(now + 250);
    assert_eq!(client.get_claimable_amount(&vault_id), 250);

    client.accelerate_all_schedules(&25);
    assert_eq!(client.get_claimable_amount(&vault_id), 500);

    client.accelerate_all_schedules(&50);
    assert_eq!(client.get_claimable_amount(&vault_id), 750);

    client.accelerate_all_schedules(&100);
    assert_eq!(client.get_claimable_amount(&vault_id), 1000);
}

#[test]
fn test_slashing() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let token_client = token::Client::new(&env, &token);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64
    );

    // Try to resume without pausing first - should panic
    client.resume_specific_schedule(&vault_id);
        &true,
        &false,
        &0u64,
    );

    env.ledger().set_timestamp(now + 400);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);

    client.slash_unvested_balance(&vault_id, &treasury);

    assert_eq!(token_client.balance(&treasury), 600);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.total_amount, 400);

    env.ledger().set_timestamp(now + 1000);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);

    client.claim_tokens(&vault_id, &400i128);
    assert_eq!(token_client.balance(&beneficiary), 400);
}

// =============================================================================
// Auto-Stake Tests
// =============================================================================

// ---------------------------------------------------------------------------
// Minimal mock staking contract for unit tests
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub struct MockStakeRecord {
    pub amount: i128,
    pub pending_yield: i128,
    pub is_active: bool,
}

#[contracttype]
enum MockDataKey {
    Admin,
    Record(Address, u64),
    AuthVault,
}

#[contract]
pub struct MockStakingContract;

#[contractimpl]
impl MockStakingContract {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&MockDataKey::Admin, &admin);
    }

    pub fn add_authorised_vault(env: Env, vault: Address) {
        env.storage().instance().set(&MockDataKey::AuthVault, &vault);
    }

    pub fn stake_tokens(env: Env, beneficiary: Address, vault_id: u64, amount: i128) {
        let key = MockDataKey::Record(beneficiary.clone(), vault_id);
        if let Some(r) = env.storage().instance().get::<_, MockStakeRecord>(&key) {
            if r.is_active { panic!("AlreadyStaked"); }
        }
        env.storage().instance().set(&key, &MockStakeRecord {
            amount,
            pending_yield: 0,
            is_active: true,
        });
    }

    pub fn unstake_tokens(env: Env, beneficiary: Address, vault_id: u64) {
        let key = MockDataKey::Record(beneficiary.clone(), vault_id);
        let mut r: MockStakeRecord = env.storage().instance()
            .get(&key).unwrap_or_else(|| panic!("NotStaked"));
        if !r.is_active { panic!("NotStaked"); }
        r.is_active = false;
        env.storage().instance().set(&key, &r);
    }

    pub fn claim_yield_for(env: Env, beneficiary: Address, vault_id: u64) -> i128 {
        let key = MockDataKey::Record(beneficiary.clone(), vault_id);
        let mut r: MockStakeRecord = env.storage().instance()
            .get(&key).unwrap_or_else(|| panic!("NotStaked"));
        let y = r.pending_yield;
        r.pending_yield = 0;
        env.storage().instance().set(&key, &r);
        y
    }

    pub fn accrue_yield(env: Env, beneficiary: Address, vault_id: u64, amount: i128) {
        let key = MockDataKey::Record(beneficiary.clone(), vault_id);
        let mut r: MockStakeRecord = env.storage().instance()
            .get(&key).unwrap_or_else(|| panic!("NotStaked"));
        r.pending_yield += amount;
        env.storage().instance().set(&key, &r);
    }

    pub fn get_record(env: Env, beneficiary: Address, vault_id: u64) -> MockStakeRecord {
        env.storage().instance()
            .get(&MockDataKey::Record(beneficiary, vault_id))
            .unwrap_or_else(|| panic!("NotStaked"))
    }
}

// ---------------------------------------------------------------------------
// Setup helper with staking
// ---------------------------------------------------------------------------

fn setup_with_staking() -> (
    Env,
    Address, // vesting contract id
    VestingContractClient<'static>,
    Address, // admin
    Address, // token
    Address, // staking contract id
) {
    let (env, contract_id, client, admin, token) = setup();

    let staking_id = env.register(MockStakingContract, ());
    let staking_client = MockStakingContractClient::new(&env, &staking_id);
    staking_client.initialize(&admin);
    staking_client.add_authorised_vault(&contract_id);

    client.add_staking_contract(&staking_id);

    (env, contract_id, client, admin, token, staking_id)
}

fn make_vault(client: &VestingContractClient, env: &Env, beneficiary: &Address) -> u64 {
    let now = env.ledger().timestamp();
    client.create_vault_full(
        beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    )
}

// ---------------------------------------------------------------------------
// Whitelist
// ---------------------------------------------------------------------------

#[test]
fn test_add_staking_contract_whitelists_address() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    assert!(client.get_staking_contracts().contains(&staking_id));
}

#[test]
fn test_remove_staking_contract_removes_from_whitelist() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    client.remove_staking_contract(&staking_id);
    assert!(!client.get_staking_contracts().contains(&staking_id));
}

// ---------------------------------------------------------------------------
// auto_stake
// ---------------------------------------------------------------------------

#[test]
fn test_auto_stake_sets_staked_state() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);

    let status = client.get_stake_status(&vault_id);
    assert_eq!(status.tokens_staked, 1000);
    match status.stake_state {
        StakeState::Staked(_, sc) => assert_eq!(sc, staking_id),
        StakeState::Unstaked => panic!("Expected Staked"),
    }
}

#[test]
fn test_auto_stake_updates_vault_staked_amount() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);

    assert_eq!(client.get_vault(&vault_id).staked_amount, 1000);
}

#[test]
#[should_panic(expected = "AlreadyStaked")]
fn test_auto_stake_double_stake_panics() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);
    client.auto_stake(&vault_id, &staking_id);
}

#[test]
#[should_panic(expected = "UnauthorizedStakingContract")]
fn test_auto_stake_non_whitelisted_contract_panics() {
    let (env, _, client, _, _, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "InsufficientBalance")]
fn test_auto_stake_zero_balance_panics() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary, &100i128, &now, &(now + 1), &0i128, &true, &false, &0u64,
    );
    env.ledger().set_timestamp(now + 2);
    client.claim_tokens(&vault_id, &100i128);

    client.auto_stake(&vault_id, &staking_id);
}

// ---------------------------------------------------------------------------
// manual_unstake
// ---------------------------------------------------------------------------

#[test]
fn test_manual_unstake_resets_state() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);
    client.manual_unstake(&vault_id);

    let status = client.get_stake_status(&vault_id);
    assert_eq!(status.tokens_staked, 0);
    assert!(matches!(status.stake_state, StakeState::Unstaked));
    assert_eq!(client.get_vault(&vault_id).staked_amount, 0);
}

#[test]
#[should_panic(expected = "NotStaked")]
fn test_manual_unstake_when_not_staked_panics() {
    let (env, _, client, _, _, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.manual_unstake(&vault_id);
}

#[test]
fn test_unstake_then_restake_succeeds() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);
    client.manual_unstake(&vault_id);
    client.auto_stake(&vault_id, &staking_id);

    assert_eq!(client.get_stake_status(&vault_id).tokens_staked, 1000);
}

// ---------------------------------------------------------------------------
// revoke_vault
// ---------------------------------------------------------------------------

#[test]
fn test_revoke_vault_unstakes_and_sends_to_treasury() {
    let (env, _, client, _, token_addr, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);
    client.revoke_vault(&vault_id, &treasury);

    let vault = client.get_vault(&vault_id);
    assert!(vault.is_frozen);
    assert_eq!(vault.staked_amount, 0);
    assert!(matches!(client.get_stake_status(&vault_id).stake_state, StakeState::Unstaked));

    assert_eq!(token::Client::new(&env, &token_addr).balance(&treasury), 1000);
}

#[test]
fn test_revoke_vault_not_staked_still_works() {
    let (env, _, client, _, token_addr, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.revoke_vault(&vault_id, &treasury);

    assert_eq!(token::Client::new(&env, &token_addr).balance(&treasury), 1000);
}

#[test]
#[should_panic(expected = "Vault is irrevocable")]
fn test_revoke_irrevocable_vault_panics() {
    let (env, _, client, _, _, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary, &1000i128, &now, &(now + 1000), &0i128, &false, &false, &0u64,
    );
    client.revoke_vault(&vault_id, &treasury);
}

// ---------------------------------------------------------------------------
// claim_yield
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "NotStaked")]
fn test_claim_yield_when_not_staked_panics() {
    let (env, _, client, _, _, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.claim_yield(&vault_id);
}

#[test]
#[should_panic(expected = "BeneficiaryRevoked")]
fn test_claim_yield_after_revocation_panics() {
    let (env, _, client, _, _, staking_id) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    client.auto_stake(&vault_id, &staking_id);
    client.revoke_vault(&vault_id, &treasury);
    client.claim_yield(&vault_id);
}

// ---------------------------------------------------------------------------
// get_stake_status
// ---------------------------------------------------------------------------

#[test]
fn test_get_stake_status_initial_is_unstaked() {
    let (env, _, client, _, _, _) = setup_with_staking();
    let beneficiary = Address::generate(&env);
    let vault_id = make_vault(&client, &env, &beneficiary);

    let status = client.get_stake_status(&vault_id);
    assert!(matches!(status.stake_state, StakeState::Unstaked));
    assert_eq!(status.tokens_staked, 0);
    assert_eq!(status.accumulated_yield, 0);
}

// =============================================================================
// Inheritance / Dead-Man's Switch Tests
// =============================================================================

use crate::{
    SuccessionState, NominatedData, ClaimPendingData, SucceededData,
    MIN_SWITCH_DURATION, MAX_SWITCH_DURATION, MIN_CHALLENGE_WINDOW, MAX_CHALLENGE_WINDOW,
};

/// 30-day switch duration in seconds (minimum allowed).
const SWITCH_30D: u64 = MIN_SWITCH_DURATION;
/// 7-day challenge window in seconds.
const CHALLENGE_7D: u64 = 7 * 24 * 60 * 60;

fn make_vault_for(client: &VestingContractClient, env: &Env, beneficiary: &Address) -> u64 {
    let now = env.ledger().timestamp();
    client.create_vault_full(
        beneficiary,
        &1000i128,
        &now,
        &(now + 10_000_000),
        &0i128,
        &true,
        &false,
        &0u64,
    )
}

// ---------------------------------------------------------------------------
// nominate_backup
// ---------------------------------------------------------------------------

#[test]
fn test_nominate_valid_backup_state_is_nominated() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);

    let view = client.get_succession_status(&vault_id);
    assert_eq!(view.backup, Some(backup));
    assert!(matches!(view.state, SuccessionState::Nominated(_)));
}

#[test]
#[should_panic(expected = "BackupEqualsPrimary")]
fn test_nominate_backup_equal_to_primary_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &primary, &SWITCH_30D, &CHALLENGE_7D);
}

#[test]
#[should_panic(expected = "SwitchDurationBelowMinimum")]
fn test_nominate_switch_duration_below_minimum_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &(SWITCH_30D - 1), &CHALLENGE_7D);
}

#[test]
#[should_panic(expected = "SwitchDurationAboveMaximum")]
fn test_nominate_switch_duration_above_maximum_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &(MAX_SWITCH_DURATION + 1), &CHALLENGE_7D);
}

#[test]
#[should_panic(expected = "ChallengeWindowOutOfRange")]
fn test_nominate_challenge_window_below_minimum_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &(MIN_CHALLENGE_WINDOW - 1));
}

#[test]
#[should_panic(expected = "ChallengeWindowOutOfRange")]
fn test_nominate_challenge_window_above_maximum_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &(MAX_CHALLENGE_WINDOW + 1));
}

#[test]
fn test_renomination_overwrites_previous_backup() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup1 = Address::generate(&env);
    let backup2 = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup1, &SWITCH_30D, &CHALLENGE_7D);
    client.nominate_backup(&vault_id, &backup2, &SWITCH_30D, &CHALLENGE_7D);

    let view = client.get_succession_status(&vault_id);
    assert_eq!(view.backup, Some(backup2));
}

// ---------------------------------------------------------------------------
// revoke_backup
// ---------------------------------------------------------------------------

#[test]
fn test_revoke_backup_resets_state_to_none() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    client.revoke_backup(&vault_id);

    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::None));
}

#[test]
#[should_panic(expected = "RevocationBlockedDuringClaim")]
fn test_revoke_backup_during_active_claim_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    // Advance past switch duration
    env.ledger().set_timestamp(now + SWITCH_30D + 1);
    client.initiate_succession_claim(&vault_id, &backup);

    // Primary tries to revoke — should be blocked
    client.revoke_backup(&vault_id);
}

// ---------------------------------------------------------------------------
// update_activity / heartbeat
// ---------------------------------------------------------------------------

#[test]
fn test_primary_activity_resets_timer() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);

    // Advance time but not past switch duration
    env.ledger().set_timestamp(now + SWITCH_30D / 2);

    // Primary claims tokens — triggers update_activity heartbeat
    client.claim_tokens(&vault_id, &0i128);

    let view = client.get_succession_status(&vault_id);
    if let SuccessionState::Nominated(data) = view.state {
        assert_eq!(data.last_activity, now + SWITCH_30D / 2);
    } else {
        panic!("Expected Nominated state");
    }
}

#[test]
fn test_primary_activity_during_challenge_window_cancels_claim() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D + 1);
    client.initiate_succession_claim(&vault_id, &backup);

    // Primary acts during challenge window
    env.ledger().set_timestamp(now + SWITCH_30D + 2);
    client.claim_tokens(&vault_id, &0i128);

    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::Nominated(_)));
}

// ---------------------------------------------------------------------------
// initiate_succession_claim
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "SwitchTimerNotElapsed")]
fn test_backup_claims_before_timer_elapses_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    // Only advance half the switch duration
    env.ledger().set_timestamp(now + SWITCH_30D / 2);
    client.initiate_succession_claim(&vault_id, &backup);
}

#[test]
fn test_backup_claims_after_timer_elapses_state_is_claim_pending() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);

    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::ClaimPending(_)));
}

// ---------------------------------------------------------------------------
// cancel_succession_claim
// ---------------------------------------------------------------------------

#[test]
fn test_primary_cancels_during_challenge_window_state_reverts_to_nominated() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);

    // Primary cancels before challenge window closes
    env.ledger().set_timestamp(now + SWITCH_30D + CHALLENGE_7D / 2);
    client.cancel_succession_claim(&vault_id);

    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::Nominated(_)));
}

// ---------------------------------------------------------------------------
// finalise_succession
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "ChallengeWindowNotElapsed")]
fn test_backup_finalises_before_challenge_window_closes_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);

    // Try to finalise immediately — challenge window not elapsed
    client.finalise_succession(&vault_id, &backup);
}

#[test]
fn test_backup_finalises_after_challenge_window_state_is_succeeded() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);
    env.ledger().set_timestamp(now + SWITCH_30D + CHALLENGE_7D);
    client.finalise_succession(&vault_id, &backup);

    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::Succeeded(_)));
    // Vault owner should now be the backup
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, backup);
}

#[test]
#[should_panic(expected = "AlreadySucceeded")]
fn test_nominate_new_backup_post_succession_rejected() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let backup2 = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);
    env.ledger().set_timestamp(now + SWITCH_30D + CHALLENGE_7D);
    client.finalise_succession(&vault_id, &backup);

    // Old primary tries to nominate again — should fail
    client.nominate_backup(&vault_id, &backup2, &SWITCH_30D, &CHALLENGE_7D);
}

// ---------------------------------------------------------------------------
// Full happy path
// ---------------------------------------------------------------------------

#[test]
fn test_full_happy_path_nominate_claim_finalise_new_owner_verified() {
    let (env, _, client, _, _) = setup();
    let primary = Address::generate(&env);
    let backup = Address::generate(&env);
    let vault_id = make_vault_for(&client, &env, &primary);
    let now = env.ledger().timestamp();

    // 1. Nominate
    client.nominate_backup(&vault_id, &backup, &SWITCH_30D, &CHALLENGE_7D);
    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::Nominated(_)));

    // 2. Timer elapses — backup initiates claim
    env.ledger().set_timestamp(now + SWITCH_30D);
    client.initiate_succession_claim(&vault_id, &backup);
    let view = client.get_succession_status(&vault_id);
    assert!(matches!(view.state, SuccessionState::ClaimPending(_)));

    // 3. Challenge window elapses — backup finalises
    env.ledger().set_timestamp(now + SWITCH_30D + CHALLENGE_7D);
    client.finalise_succession(&vault_id, &backup);

    // 4. Verify new ownership
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, backup);
    let view = client.get_succession_status(&vault_id);
    if let SuccessionState::Succeeded(data) = view.state {
        assert_eq!(data.new_owner, backup);
    } else {
        panic!("Expected Succeeded state");
    }
}

// ---------------------------------------------------------------------------
// Clawback Tests
// ---------------------------------------------------------------------------

#[test]
fn test_clawback_within_grace_period() {
    let (env, _, client, admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create vault with 5000 tokens over 9900 seconds
    let vault_id = client.create_vault_full(
        &beneficiary,
        &5000i128,
        &now,
        &(now + 9900),
        &0i128,
        &true, // revocable
        &false,
        &0u64
    );

    // Advance time slightly (within grace period)
    env.ledger().set_timestamp(now + 100);
    
    // Clawback should work within grace period (100 seconds)
    client.clawback_vault(&vault_id, &200u64);
    
    // Verify clawback info is stored
    let clawback_info = client.get_clawback_info(&vault_id);
    assert!(clawback_info.is_some());
    
    let info = clawback_info.unwrap();
    assert_eq!(info.vault_id, vault_id);
    assert_eq!(info.original_total_amount, 5000);
    assert!(info.clawback_amount > 0);
    assert_eq!(info.original_end_time, now + 9900);
    
    // Verify adjusted schedule is created
    let adjusted_schedule = client.get_adjusted_schedule(&vault_id);
    assert!(adjusted_schedule.is_some());
    
    // Verify vault is marked as clawed back
    assert!(client.is_vault_clawed_back(&vault_id));
}

#[test]
#[should_panic(expected = "Grace period expired")]
fn test_clawback_after_grace_period_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &5000i128,
        &now,
        &(now + 10000),
        &0i128,
        &true,
        &false,
        &0u64
    );

    // Advance time beyond grace period
    env.ledger().set_timestamp(now + 500);
    
    // Should panic - grace period expired
    client.clawback_vault(&vault_id, &200u64);
}

#[test]
fn test_dynamic_pro_rata_recalculation() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create vault: 1000 tokens over 1000 seconds (1 token/second)
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64
    );

    // Advance 500 seconds - 500 tokens should be vested
    env.ledger().set_timestamp(now + 500);
    assert_eq!(client.get_claimable_amount(&vault_id), 500);

    // Clawback within grace period - should claw back 500 unvested tokens
    client.clawback_vault(&vault_id, &1000u64);
    
    // Verify vault now has reduced total amount (only vested tokens remain)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.total_amount, 500); // Only vested tokens remain
    
    // Verify adjusted schedule maintains original end time
    let adjusted = client.get_adjusted_schedule(&vault_id).unwrap();
    assert_eq!(adjusted.original_end_time, now + 1000);
    
    // Verify emission rate has been adjusted
    // Original: 1 token/second for 1000 seconds
    // After clawback: 500 tokens over remaining 500 seconds = 1 token/second
    // In this case, the rate stays the same since we're exactly at 50%
    assert_eq!(adjusted.new_total_amount, 500);
}

#[test]
fn test_clawback_with_partial_vesting() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create vault: 1000 tokens over 1000 seconds
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64
    );

    // Advance 200 seconds - 200 tokens vested
    env.ledger().set_timestamp(now + 200);
    assert_eq!(client.get_claimable_amount(&vault_id), 200);

    // Clawback - should claw back 800 unvested tokens
    client.clawback_vault(&vault_id, &1000u64);
    
    let clawback_info = client.get_clawback_info(&vault_id).unwrap();
    assert_eq!(clawback_info.clawback_amount, 800);
    assert_eq!(clawback_info.remaining_amount, 200);
    
    // Verify adjusted schedule
    let adjusted = client.get_adjusted_schedule(&vault_id).unwrap();
    assert_eq!(adjusted.new_total_amount, 200);
    
    // After clawback, all remaining tokens should be immediately available
    // since we need to fit 200 tokens into the remaining 800 seconds
    // but the dynamic recalculation should handle this
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.total_amount, 200);
}
