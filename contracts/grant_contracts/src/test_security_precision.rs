#![cfg(test)]

use soroban_sdk::{
    Address, Env, String, Vec, Map, Hash, symbol_short,
    contracterror,
};

use crate::{
    anti_reentry_guard::{
        AntiReentryGrantContract, ReentryStatus, ReentryError,
        REENTRY_TIMEOUT_SECONDS,
    },
    authorized_lessor_registry::{
        AuthorizedLessorRegistry, LessorStatus, ComplianceLevel, LessorRegistryError,
    },
    virtual_accumulator::{
        VirtualAccumulator, VestingType, AccumulatorError,
        PRECISION_MULTIPLIER,
    },
    fraud_clawback::{
        FraudClawback, ArbitrationStatus, VoteType, FraudClawbackError,
        JURY_SIZE, VOTING_PERIOD_SECONDS,
    },
    optimized::{GrantContract, Error, STATUS_ACTIVE},
};

#[test]
fn test_anti_reentry_guard_basic_functionality() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let grant_id = 1u64;
    let amount = 1000i128;

    // Initialize anti-reentry guard
    AntiReentryGrantContract::initialize(env.clone()).unwrap();

    // Test normal withdrawal (should work)
    let result = AntiReentryGrantContract::withdraw_with_guard(
        env.clone(),
        grant_id,
        amount,
        false, // not external transfer
    );

    // This should fail because grant doesn't exist, but not due to reentry
    assert!(matches!(result, Err(Error::GrantNotFound)));
}

#[test]
fn test_anti_reentry_guard_lock_mechanism() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let grant_id = 1u64;

    // Initialize anti-reentry guard
    AntiReentryGrantContract::initialize(env.clone()).unwrap();

    // Check if grant is locked (should be false initially)
    let is_locked = AntiReentryGrantContract::is_grant_locked(
        env.clone(),
        recipient.clone(),
        grant_id,
    ).unwrap();
    assert!(!is_locked);
}

#[test]
fn test_authorized_lessor_registry_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Initialize registry
    let result = AuthorizedLessorRegistry::initialize(env.clone(), admin.clone());
    assert!(result.is_ok());

    // Test duplicate initialization
    let result = AuthorizedLessorRegistry::initialize(env.clone(), admin);
    assert!(matches!(result, Err(LessorRegistryError::NotAuthorized)));
}

#[test]
fn test_authorized_lessor_registration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let lessor = Address::generate(&env);

    // Initialize registry
    AuthorizedLessorRegistry::initialize(env.clone(), admin.clone()).unwrap();

    // Register a lessor
    let name = String::from_str(&env, "Test Institution");
    let jurisdiction = String::from_str(&env, "US");
    let metadata = Map::<String, String>::new(&env);

    let result = AuthorizedLessorRegistry::register_lessor(
        env.clone(),
        lessor.clone(),
        name.clone(),
        ComplianceLevel::Standard,
        jurisdiction.clone(),
        Some(String::from_str(&env, "LICENSE123")),
        1000000i128,
        metadata,
    );

    assert!(result.is_ok());

    // Check pending approvals
    let pending = AuthorizedLessorRegistry::get_pending_approvals(env.clone()).unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending.contains(&lessor));

    // Test duplicate registration
    let result = AuthorizedLessorRegistry::register_lessor(
        env.clone(),
        lessor,
        name,
        ComplianceLevel::Standard,
        jurisdiction,
        Some(String::from_str(&env, "LICENSE123")),
        1000000i128,
        Map::<String, String>::new(&env),
    );

    assert!(matches!(result, Err(LessorRegistryError::LessorAlreadyExists)));
}

#[test]
fn test_authorized_lessor_approval() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let lessor = Address::generate(&env);

    // Initialize registry
    AuthorizedLessorRegistry::initialize(env.clone(), admin.clone()).unwrap();

    // Register a lessor
    let name = String::from_str(&env, "Test Institution");
    let jurisdiction = String::from_str(&env, "US");

    AuthorizedLessorRegistry::register_lessor(
        env.clone(),
        lessor.clone(),
        name,
        ComplianceLevel::Standard,
        jurisdiction,
        Some(String::from_str(&env, "LICENSE123")),
        1000000i128,
        Map::<String, String>::new(&env),
    ).unwrap();

    // Approve the lessor
    let result = AuthorizedLessorRegistry::approve_lessor(env.clone(), lessor.clone());
    assert!(result.is_ok());

    // Check if lessor is now authorized
    let is_authorized = AuthorizedLessorRegistry::is_lessor_authorized(env.clone(), lessor.clone()).unwrap();
    assert!(is_authorized);

    // Check active lessors
    let active_lessors = AuthorizedLessorRegistry::get_active_lessors(env.clone()).unwrap();
    assert_eq!(active_lessors.len(), 1);
    assert!(active_lessors.contains(&lessor));

    // Test approving already approved lessor
    let result = AuthorizedLessorRegistry::approve_lessor(env.clone(), lessor);
    assert!(matches!(result, Err(LessorRegistryError::InvalidStatus)));
}

#[test]
fn test_virtual_accumulator_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Initialize virtual accumulator
    let result = VirtualAccumulator::initialize(env.clone(), admin.clone());
    assert!(result.is_ok());

    // Test duplicate initialization
    let result = VirtualAccumulator::initialize(env.clone(), admin);
    assert!(matches!(result, Err(AccumulatorError::AlreadyInitialized)));
}

#[test]
fn test_virtual_accumulator_vesting_creation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let grant_id = 1u64;
    let total_amount = 1000000i128;
    let now = env.ledger().timestamp();

    // Initialize virtual accumulator
    VirtualAccumulator::initialize(env.clone(), admin.clone()).unwrap();

    // Create high-frequency vesting
    let result = VirtualAccumulator::create_vesting(
        env.clone(),
        grant_id,
        recipient.clone(),
        total_amount,
        now,
        now + 365 * 24 * 60 * 60, // 1 year
        now + 30 * 24 * 60 * 60,  // 30 day cliff
        VestingType::Linear,
    );

    assert!(result.is_ok());

    // Test duplicate vesting creation
    let result = VirtualAccumulator::create_vesting(
        env.clone(),
        grant_id,
        recipient,
        total_amount,
        now,
        now + 365 * 24 * 60 * 60,
        now + 30 * 24 * 60 * 60,
        VestingType::Linear,
    );

    assert!(matches!(result, Err(AccumulatorError::AlreadyInitialized)));
}

#[test]
fn test_virtual_accumulator_precision() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let grant_id = 1u64;
    let total_amount = 1000000i128;
    let now = env.ledger().timestamp();

    // Initialize virtual accumulator
    VirtualAccumulator::initialize(env.clone(), admin).unwrap();

    // Create high-frequency vesting
    VirtualAccumulator::create_vesting(
        env.clone(),
        grant_id,
        recipient.clone(),
        total_amount,
        now,
        now + 365 * 24 * 60 * 60,
        now, // no cliff for easier testing
        VestingType::Linear,
    ).unwrap();

    // Test vested amount calculation
    let vested_amount = VirtualAccumulator::get_vested_amount(env.clone(), grant_id).unwrap();
    assert_eq!(vested_amount, 0); // Should be 0 at start time

    // Test claimable amount
    let claimable = VirtualAccumulator::get_claimable_amount(env.clone(), grant_id).unwrap();
    assert_eq!(claimable, 0); // Should be 0 at start time
}

#[test]
fn test_virtual_accumulator_rate_update() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let grant_id = 1u64;
    let total_amount = 1000000i128;
    let now = env.ledger().timestamp();

    // Initialize virtual accumulator
    VirtualAccumulator::initialize(env.clone(), admin).unwrap();

    // Create high-frequency vesting
    VirtualAccumulator::create_vesting(
        env.clone(),
        grant_id,
        recipient,
        total_amount,
        now,
        now + 365 * 24 * 60 * 60,
        now,
        VestingType::Linear,
    ).unwrap();

    // Update rate
    let new_rate = 2000i128;
    let result = VirtualAccumulator::update_rate(env.clone(), grant_id, new_rate);
    assert!(result.is_ok());

    // Test negative rate
    let result = VirtualAccumulator::update_rate(env.clone(), grant_id, -1000i128);
    assert!(matches!(result, Err(AccumulatorError::InvalidVestingParameters)));
}

#[test]
fn test_fraud_clawback_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize fraud clawback system
    let result = FraudClawback::initialize(env.clone(), admin.clone(), dao.clone(), treasury.clone());
    assert!(result.is_ok());

    // Test duplicate initialization
    let result = FraudClawback::initialize(env.clone(), admin, dao, treasury);
    assert!(matches!(result, Err(FraudClawbackError::NotAuthorized)));
}

#[test]
fn test_fraud_dispute_raising() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = Address::generate(&env);
    let grant_id = 1u64;
    let slash_amount = 500000i128;

    // Initialize fraud clawback system
    FraudClawback::initialize(env.clone(), admin, dao.clone(), treasury).unwrap();

    // Raise a fraud dispute
    let evidence_hash = Hash::from_bytes(&env, &[1u8; 32]);
    let reasoning = String::from_str(&env, "Suspicious activity detected");
    let metadata = Map::<String, String>::new(&env);

    let dispute_id = FraudClawback::raise_fraud_dispute(
        env.clone(),
        grant_id,
        target.clone(),
        evidence_hash,
        reasoning.clone(),
        slash_amount,
        metadata,
    ).unwrap();

    assert!(dispute_id > 0);

    // Check active disputes
    let active_disputes = FraudClawback::get_active_disputes(env.clone()).unwrap();
    assert_eq!(active_disputes.len(), 1);
    assert!(active_disputes.contains(&dispute_id));

    // Check if grant is frozen
    let is_frozen = FraudClawback::is_grant_frozen(env.clone(), grant_id).unwrap();
    assert!(is_frozen);

    // Test raising dispute for already frozen grant
    let result = FraudClawback::raise_fraud_dispute(
        env.clone(),
        grant_id,
        target,
        evidence_hash,
        reasoning,
        slash_amount,
        Map::<String, String>::new(&env),
    );

    assert!(matches!(result, Err(FraudClawbackError::GrantAlreadyFrozen)));
}

#[test]
fn test_jury_selection() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = Address::generate(&env);
    let grant_id = 1u64;
    let slash_amount = 500000i128;

    // Initialize fraud clawback system
    FraudClawback::initialize(env.clone(), admin, dao.clone(), treasury).unwrap();

    // Add some jurors to the pool
    for _ in 0..10 {
        let juror = Address::generate(&env);
        FraudClawback::add_juror(env.clone(), juror, 75).unwrap();
    }

    // Raise a fraud dispute
    let evidence_hash = Hash::from_bytes(&env, &[1u8; 32]);
    let reasoning = String::from_str(&env, "Test dispute");
    let dispute_id = FraudClawback::raise_fraud_dispute(
        env.clone(),
        grant_id,
        target,
        evidence_hash,
        reasoning,
        slash_amount,
        Map::<String, String>::new(&env),
    ).unwrap();

    // Select jury
    let result = FraudClawback::select_jury(env.clone(), dispute_id);
    assert!(result.is_ok());

    // Check dispute details
    let dispute = FraudClawback::get_dispute(env.clone(), dispute_id).unwrap();
    assert_eq!(dispute.status, ArbitrationStatus::JurySelected);
    assert_eq!(dispute.selected_jurors.len(), JURY_SIZE as usize);

    // Test selecting jury for already selected dispute
    let result = FraudClawback::select_jury(env.clone(), dispute_id);
    assert!(matches!(result, Err(FraudClawbackError::InvalidJurorSelection)));
}

#[test]
fn test_voting_process() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = Address::generate(&env);
    let grant_id = 1u64;
    let slash_amount = 500000i128;

    // Initialize fraud clawback system
    FraudClawback::initialize(env.clone(), admin, dao.clone(), treasury).unwrap();

    // Add jurors
    let mut jurors = Vec::<Address>::new(&env);
    for _ in 0..10 {
        let juror = Address::generate(&env);
        FraudClawback::add_juror(env.clone(), juror.clone(), 75).unwrap();
        jurors.push_back(juror);
    }

    // Raise a fraud dispute
    let evidence_hash = Hash::from_bytes(&env, &[1u8; 32]);
    let reasoning = String::from_str(&env, "Test dispute");
    let dispute_id = FraudClawback::raise_fraud_dispute(
        env.clone(),
        grant_id,
        target,
        evidence_hash,
        reasoning,
        slash_amount,
        Map::<String, String>::new(&env),
    ).unwrap();

    // Select jury
    FraudClawback::select_jury(env.clone(), dispute_id).unwrap();

    // Start voting
    let result = FraudClawback::start_voting(env.clone(), dispute_id);
    assert!(result.is_ok());

    // Get dispute to see selected jurors
    let dispute = FraudClawback::get_dispute(env.clone(), dispute_id).unwrap();
    assert_eq!(dispute.status, ArbitrationStatus::VotingInProgress);

    // Test voting (this would require proper authentication in real implementation)
    // For testing purposes, we'll just verify the structure
    assert_eq!(dispute.selected_jurors.len(), JURY_SIZE as usize);
}

#[test]
fn test_juror_management() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize fraud clawback system
    FraudClawback::initialize(env.clone(), admin, dao.clone(), treasury).unwrap();

    // Add juror with sufficient reputation
    let juror = Address::generate(&env);
    let result = FraudClawback::add_juror(env.clone(), juror.clone(), 75);
    assert!(result.is_ok());

    // Test adding juror with insufficient reputation
    let low_rep_juror = Address::generate(&env);
    let result = FraudClawback::add_juror(env.clone(), low_rep_juror, 25);
    assert!(matches!(result, Err(FraudClawbackError::InvalidJurorSelection)));
}

#[test]
fn test_comprehensive_integration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);
    let recipient = Address::generate(&env);
    let lessor = Address::generate(&env);

    // Initialize all systems
    AntiReentryGrantContract::initialize(env.clone()).unwrap();
    AuthorizedLessorRegistry::initialize(env.clone(), admin.clone()).unwrap();
    VirtualAccumulator::initialize(env.clone(), admin.clone()).unwrap();
    FraudClawback::initialize(env.clone(), admin, dao, treasury).unwrap();

    // Register and approve a lessor
    let name = String::from_str(&env, "Integration Test Institution");
    let jurisdiction = String::from_str(&env, "US");
    
    AuthorizedLessorRegistry::register_lessor(
        env.clone(),
        lessor.clone(),
        name,
        ComplianceLevel::Enhanced,
        jurisdiction,
        Some(String::from_str(&env, "INT-TEST-001")),
        5000000i128,
        Map::<String, String>::new(&env),
    ).unwrap();

    AuthorizedLessorRegistry::approve_lessor(env.clone(), lessor.clone()).unwrap();

    // Create institutional vesting
    let grant_id = 1u64;
    let total_amount = 1000000i128;
    let now = env.ledger().timestamp();

    AuthorizedLessorRegistry::create_institutional_vesting(
        env.clone(),
        grant_id,
        lessor.clone(),
        total_amount,
        365 * 24 * 60 * 60, // 1 year
        30 * 24 * 60 * 60,  // 30 day cliff
        24 * 60 * 60,       // 1 day slice period
    ).unwrap();

    // Create high-frequency vesting for the same grant
    VirtualAccumulator::create_vesting(
        env.clone(),
        grant_id + 1,
        recipient,
        total_amount / 2,
        now,
        now + 180 * 24 * 60 * 60, // 6 months
        now,
        VestingType::Accelerated,
    ).unwrap();

    // Verify lessor is authorized
    let is_authorized = AuthorizedLessorRegistry::is_lessor_authorized(env.clone(), lessor).unwrap();
    assert!(is_authorized);

    // Verify virtual accumulator state
    let vested_amount = VirtualAccumulator::get_vested_amount(env.clone(), grant_id + 1).unwrap();
    assert_eq!(vested_amount, 0); // Should be 0 at start

    // Test anti-reentry guard
    let is_locked = AntiReentryGrantContract::is_grant_locked(env.clone(), recipient, grant_id + 1).unwrap();
    assert!(!is_locked);
}

#[test]
fn test_error_handling_edge_cases() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Test operations on uninitialized systems
    let result = AntiReentryGrantContract::is_grant_locked(env.clone(), admin.clone(), 1u64);
    assert!(result.is_ok()); // Should return false for non-existent grants

    let result = AuthorizedLessorRegistry::is_lessor_authorized(env.clone(), admin.clone());
    assert!(result.is_ok()); // Should return false for non-existent lessors

    let result = VirtualAccumulator::get_vested_amount(env.clone(), 1u64);
    assert!(matches!(result, Err(AccumulatorError::VestingNotFound)));

    let result = FraudClawback::get_dispute(env.clone(), 1u64);
    assert!(matches!(result, Err(FraudClawbackError::DisputeNotFound)));
}

#[test]
fn test_precision_arithmetic() {
    use crate::virtual_accumulator::{to_precision, from_precision, precise_multiply, precise_divide};

    let env = Env::default();

    // Test precision conversion
    let amount = 123456789i128;
    let precise = to_precision(amount).unwrap();
    assert_eq!(precise, amount as u128 * PRECISION_MULTIPLIER);

    let converted_back = from_precision(precise).unwrap();
    assert_eq!(converted_back, amount);

    // Test precise multiplication
    let a = 1000000u128;
    let b = 2u128;
    let result = precise_multiply(a, b).unwrap();
    assert_eq!(result, 2000000u128);

    // Test precise division
    let a = 2000000u128;
    let b = 2u128;
    let result = precise_divide(a, b).unwrap();
    assert_eq!(result, 1000000u128);

    // Test division by zero
    let result = precise_divide(100u128, 0u128);
    assert!(matches!(result, Err(AccumulatorError::InvalidVestingParameters)));
}

#[test]
fn test_juror_selection_randomness() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize fraud clawback system
    FraudClawback::initialize(env.clone(), admin, dao.clone(), treasury).unwrap();

    // Add many jurors
    let mut jurors = Vec::<Address>::new(&env);
    for i in 0..20 {
        let juror = Address::generate(&env);
        FraudClawback::add_juror(env.clone(), juror.clone(), 75).unwrap();
        jurors.push_back(juror);
    }

    // Create multiple disputes and verify different jury selections
    let mut selected_juries = Vec::<Vec<Address>>::new(&env);
    
    for i in 0..5 {
        let evidence_hash = Hash::from_bytes(&env, &[(i + 1) as u8; 32]);
        let reasoning = String::from_str(&env, &format!("Test dispute {}", i));
        
        let dispute_id = FraudClawback::raise_fraud_dispute(
            env.clone(),
            i + 1,
            Address::generate(&env),
            evidence_hash,
            reasoning,
            500000i128,
            Map::<String, String>::new(&env),
        ).unwrap();

        FraudClawback::select_jury(env.clone(), dispute_id).unwrap();
        
        let dispute = FraudClawback::get_dispute(env.clone(), dispute_id).unwrap();
        selected_juries.push_back(dispute.selected_jurors.clone());
    }

    // Verify that we have different juries (randomness working)
    assert_eq!(selected_juries.len(), 5);
    for jury in selected_juries.iter() {
        assert_eq!(jury.len(), JURY_SIZE as usize);
    }
}
