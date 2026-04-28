#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as TestAddress, Ledger as TestLedger},
    Address, Env, String,
};

use crate::{
    anti_reentry_guard::{
        AntiReentryContract, ReentryGuard, ReentryProtection, ReentryError, REENTRY_GUARD_ACTIVE,
    },
    virtual_accumulator::{
        VirtualAccumulatorContract, AccumulatorConfig, AccumulatorState, UserAccumulatorState,
        AccumulatorError, VirtualAccumulator, PRECISION_MULTIPLIER,
    },
    authorized_lessor_registry::{
        AuthorizedLessorRegistryContract, AuthorizedLessor, InstitutionalData, LessorRegistryError,
        LessorRegistry, LESSOR_STATUS_APPROVED, LESSOR_STATUS_PENDING, TIER_STANDARD,
    },
    fraud_clawback::{
        FraudClawbackContract, FraudDispute, FraudResolution, SecurityCouncilMember, FraudError,
        FraudClawback, DISPUTE_STATUS_FROZEN, DISPUTE_STATUS_JURY_SELECTED, JURY_SIZE,
    },
};

#[test]
fn test_anti_reentry_guard() {
    let env = Env::new();
    let contract_id = env.register_contract(None, AntiReentryContract);
    let client = AntiReentryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Test reentry protection
    assert!(!ReentryProtection::is_guard_active(&env, &user));

    // Set guard active
    ReentryProtection::set_guard_active(&env, &user).unwrap();
    assert!(ReentryProtection::is_guard_active(&env, &user));

    // Test duplicate guard activation
    assert_eq!(
        ReentryProtection::set_guard_active(&env, &user),
        Err(ReentryError::GuardAlreadyActive)
    );

    // Clear guard
    ReentryProtection::clear_guard(&env, &user).unwrap();
    assert!(!ReentryProtection::is_guard_active(&env, &user));

    // Test clearing non-existent guard
    assert_eq!(
        ReentryProtection::clear_guard(&env, &user),
        Err(ReentryError::GuardNotActive)
    );

    // Test protected execution
    let result = ReentryProtection::execute_with_protection(&env, &user, |_env| {
        Ok(42i32)
    });
    assert_eq!(result, Ok(42i32));
    assert!(!ReentryProtection::is_guard_active(&env, &user)); // Guard should be cleared
}

#[test]
fn test_virtual_accumulator() {
    let env = Env::new();
    let contract_id = env.register_contract(None, VirtualAccumulatorContract);
    let client = VirtualAccumulatorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize accumulator
    client.initialize(&admin).unwrap();

    // Test config
    let config = client.get_config().unwrap();
    assert_eq!(config.precision, PRECISION_MULTIPLIER);
    assert_eq!(config.max_periods, 1000);

    // Create user accumulator
    client.create_user_accumulator(&admin, &user, &1000u128).unwrap();

    // Test user state
    let user_state = client.get_user_state(&user).unwrap();
    assert_eq!(user_state.rate_multiplier, 1000);
    assert_eq!(user_state.accumulated_balance, 0);

    // Test balance calculation
    env.ledger().set_timestamp(1000); // Set timestamp
    let balance = client.get_balance(&user).unwrap();
    assert_eq!(balance, 0); // No time has passed yet

    // Advance time
    env.ledger().set_timestamp(2000); // 1000 seconds later
    let balance = client.get_balance(&user).unwrap();
    assert!(balance > 0);

    // Test claiming
    let claim_amount = 500u128;
    let claimed = client.claim(&user, &claim_amount).unwrap();
    assert_eq!(claimed, claim_amount);

    // Test insufficient balance
    let large_claim = 1000000u128;
    assert_eq!(
        client.claim(&user, &large_claim),
        Err(AccumulatorError::InsufficientBalance)
    );
}

#[test]
fn test_authorized_lessor_registry() {
    let env = Env::new();
    let contract_id = env.register_contract(None, AuthorizedLessorRegistryContract);
    let client = AuthorizedLessorRegistryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let lessor = Address::generate(&env);

    // Initialize registry
    client.initialize(&admin).unwrap();

    // Register lessor
    let name = String::from_str(&env, "Test Lessor");
    let institutional_data = InstitutionalData {
        institution_type: String::from_str(&env, "bank"),
        registration_number: String::from_str(&env, "REG123"),
        jurisdiction: String::from_str(&env, "US"),
        regulatory_compliance: true,
        audit_report_hash: None,
        risk_rating: 5,
    };

    client.register_lessor(
        &lessor,
        &name,
        &TIER_STANDARD,
        &1000000i128,
        Some(institutional_data.clone()),
    ).unwrap();

    // Test lessor info
    let lessor_info = client.get_lessor(&lessor).unwrap();
    assert_eq!(lessor_info.name, name);
    assert_eq!(lessor_info.tier, TIER_STANDARD);
    assert!(lessor_info.institutional_data.is_some());

    // Test pending status
    assert!(has_lessor_status(lessor_info.status_mask, LESSOR_STATUS_PENDING));
    assert!(!has_lessor_status(lessor_info.status_mask, LESSOR_STATUS_APPROVED));

    // Approve lessor
    client.approve_lessor(&admin, &lessor).unwrap();

    // Test approved status
    let lessor_info = client.get_lessor(&lessor).unwrap();
    assert!(has_lessor_status(lessor_info.status_mask, LESSOR_STATUS_APPROVED));
    assert!(!has_lessor_status(lessor_info.status_mask, LESSOR_STATUS_PENDING));

    // Test authorization check
    assert!(client.is_authorized(&lessor).unwrap());

    // Test allocation update
    client.update_allocation(&lessor, &500000i128).unwrap();
    let lessor_info = client.get_lessor(&lessor).unwrap();
    assert_eq!(lessor_info.current_allocation, 500000);

    // Test exceeding allocation
    assert_eq!(
        client.update_allocation(&lessor, &2000000i128),
        Err(LessorRegistryError::ExceedsAllocation)
    );

    // Test suspension
    client.suspend_lessor(&admin, &lessor).unwrap();
    assert!(!client.is_authorized(&lessor).unwrap()); // Should not be authorized when suspended

    // Test revocation
    client.revoke_lessor(&admin, &lessor).unwrap();
    assert!(!client.is_authorized(&lessor).unwrap()); // Should not be authorized when revoked
}

#[test]
fn test_fraud_clawback() {
    let env = Env::new();
    let contract_id = env.register_contract(None, FraudClawbackContract);
    let client = FraudClawbackContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);
    let juror4 = Address::generate(&env);
    let juror5 = Address::generate(&env);

    // Initialize fraud clawback system
    client.initialize(&admin, &dao).unwrap();

    // Add security council members
    client.add_security_council_member(&admin, &juror1).unwrap();
    client.add_security_council_member(&admin, &juror2).unwrap();
    client.add_security_council_member(&admin, &juror3).unwrap();
    client.add_security_council_member(&admin, &juror4).unwrap();
    client.add_security_council_member(&admin, &juror5).unwrap();

    // Test security council
    let council = client.get_security_council().unwrap();
    assert_eq!(council.len(), 5);

    // Raise fraud dispute
    let grant_id = 12345u64;
    let evidence_hash = String::from_str(&env, "hash123");
    let description = String::from_str(&env, "Suspicious activity detected");

    let dispute_id = client.raise_fraud_dispute(
        &dao,
        &grant_id,
        &beneficiary,
        Some(evidence_hash.clone()),
        &description,
    ).unwrap();

    // Test grant is frozen
    assert!(client.is_grant_frozen(&grant_id));

    // Test dispute info
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.target_grant_id, grant_id);
    assert_eq!(dispute.target_beneficiary, beneficiary);
    assert_eq!(dispute.jury_members.len(), JURY_SIZE as usize);
    assert!(has_dispute_status(dispute.status_mask, DISPUTE_STATUS_FROZEN));
    assert!(has_dispute_status(dispute.status_mask, DISPUTE_STATUS_JURY_SELECTED));

    // Test voting
    let jury_members = dispute.jury_members.clone();
    
    // First juror votes for fraud
    client.cast_jury_vote(&jury_members.get(0).unwrap(), &dispute_id, &true, None).unwrap();
    
    // Second juror votes for fraud
    client.cast_jury_vote(&jury_members.get(1).unwrap(), &dispute_id, &true, None).unwrap();
    
    // Third juror votes for fraud (should reach threshold)
    client.cast_jury_vote(&jury_members.get(2).unwrap(), &dispute_id, &true, None).unwrap();

    // Check if dispute is resolved
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert!(dispute.resolved_timestamp.is_some());
    assert!(dispute.resolution.is_fraud_confirmed);

    // Test that grant is unfrozen after resolution
    assert!(!client.is_grant_frozen(&grant_id));

    // Test duplicate voting
    assert_eq!(
        client.cast_jury_vote(&jury_members.get(0).unwrap(), &dispute_id, &false, None),
        Err(FraudError::AlreadyVoted)
    );

    // Test voting on resolved dispute
    assert_eq!(
        client.cast_jury_vote(&jury_members.get(3).unwrap(), &dispute_id, &false, None),
        Err(FraudError::VotingPeriodExpired)
    );
}

#[test]
fn test_fraud_clawback_dismissal() {
    let env = Env::new();
    let contract_id = env.register_contract(None, FraudClawbackContract);
    let client = FraudClawbackContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);
    let juror4 = Address::generate(&env);
    let juror5 = Address::generate(&env);

    // Initialize system
    client.initialize(&admin, &dao).unwrap();

    // Add security council members
    client.add_security_council_member(&admin, &juror1).unwrap();
    client.add_security_council_member(&admin, &juror2).unwrap();
    client.add_security_council_member(&admin, &juror3).unwrap();
    client.add_security_council_member(&admin, &juror4).unwrap();
    client.add_security_council_member(&admin, &juror5).unwrap();

    // Raise dispute
    let grant_id = 12346u64;
    let dispute_id = client.raise_fraud_dispute(
        &dao,
        &grant_id,
        &beneficiary,
        None,
        &String::from_str(&env, "Test dispute"),
    ).unwrap();

    let dispute = client.get_dispute(&dispute_id).unwrap();
    let jury_members = dispute.jury_members.clone();

    // Vote against fraud (2 votes)
    client.cast_jury_vote(&jury_members.get(0).unwrap(), &dispute_id, &false, None).unwrap();
    client.cast_jury_vote(&jury_members.get(1).unwrap(), &dispute_id, &false, None).unwrap();

    // Vote for fraud (1 vote)
    client.cast_jury_vote(&jury_members.get(2).unwrap(), &dispute_id, &true, None).unwrap();

    // Should not be resolved yet (only 3 votes, but need 3 for fraud)
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert!(dispute.resolved_timestamp.is_none());

    // Final votes against fraud (2 more)
    client.cast_jury_vote(&jury_members.get(3).unwrap(), &dispute_id, &false, None).unwrap();
    client.cast_jury_vote(&jury_members.get(4).unwrap(), &dispute_id, &false, None).unwrap();

    // Now should be resolved with dismissal
    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert!(dispute.resolved_timestamp.is_some());
    assert!(!dispute.resolution.is_fraud_confirmed);
    assert_eq!(dispute.votes_against, 4);
    assert_eq!(dispute.votes_for_fraud, 1);
}

#[test]
fn test_overlapping_disputes() {
    let env = Env::new();
    let contract_id = env.register_contract(None, FraudClawbackContract);
    let client = FraudClawbackContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let dao = Address::generate(&env);
    let beneficiary1 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);
    let juror = Address::generate(&env);

    // Initialize system
    client.initialize(&admin, &dao).unwrap();

    // Add security council member
    client.add_security_council_member(&admin, &juror).unwrap();

    // Create multiple disputes for different grants
    let grant1 = 1001u64;
    let grant2 = 1002u64;
    let grant3 = 1003u64;

    let dispute1 = client.raise_fraud_dispute(
        &dao,
        &grant1,
        &beneficiary1,
        None,
        &String::from_str(&env, "Dispute 1"),
    ).unwrap();

    let dispute2 = client.raise_fraud_dispute(
        &dao,
        &grant2,
        &beneficiary2,
        None,
        &String::from_str(&env, "Dispute 2"),
    ).unwrap();

    let dispute3 = client.raise_fraud_dispute(
        &dao,
        &grant3,
        &beneficiary1, // Same beneficiary as dispute 1
        None,
        &String::from_str(&env, "Dispute 3"),
    ).unwrap();

    // Test all grants are frozen
    assert!(client.is_grant_frozen(&grant1));
    assert!(client.is_grant_frozen(&grant2));
    assert!(client.is_grant_frozen(&grant3));

    // Test disputes are separate
    let d1 = client.get_dispute(&dispute1).unwrap();
    let d2 = client.get_dispute(&dispute2).unwrap();
    let d3 = client.get_dispute(&dispute3).unwrap();

    assert_ne!(d1.dispute_id, d2.dispute_id);
    assert_ne!(d2.dispute_id, d3.dispute_id);
    assert_ne!(d1.dispute_id, d3.dispute_id);

    assert_eq!(d1.target_grant_id, grant1);
    assert_eq!(d2.target_grant_id, grant2);
    assert_eq!(d3.target_grant_id, grant3);

    // Test that same beneficiary can have multiple disputes
    assert_eq!(d1.target_beneficiary, beneficiary1);
    assert_eq!(d2.target_beneficiary, beneficiary2);
    assert_eq!(d3.target_beneficiary, beneficiary1);
}

#[test]
fn test_virtual_accumulator_precision() {
    let env = Env::new();
    let contract_id = env.register_contract(None, VirtualAccumulatorContract);
    let client = VirtualAccumulatorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Initialize with custom config
    client.initialize(&admin).unwrap();

    // Create user with very high rate
    let high_rate = 1_000_000_000u128; // 1 billion tokens per second
    client.create_user_accumulator(&admin, &user, &high_rate).unwrap();

    // Test precision with small time increments
    env.ledger().set_timestamp(1000);
    env.ledger().set_timestamp(1001); // 1 second

    let balance = client.get_balance(&user).unwrap();
    assert_eq!(balance, high_rate);

    // Test with fractional time (should handle precision correctly)
    env.ledger().set_timestamp(1002); // Another second
    let balance = client.get_balance(&user).unwrap();
    assert_eq!(balance, high_rate * 2);

    // Test claiming partial amounts
    let claim_amount = high_rate / 2; // Half of accumulated
    let claimed = client.claim(&user, &claim_amount).unwrap();
    assert_eq!(claimed, claim_amount);

    // Remaining balance should be correct
    let remaining = client.get_balance(&user).unwrap();
    assert_eq!(remaining, high_rate * 2 - claim_amount);
}

#[test]
fn test_anti_reentry_nested_calls() {
    let env = Env::new();
    let contract_id = env.register_contract(None, AntiReentryContract);
    let client = AntiReentryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // Test nested reentry protection
    let result = ReentryProtection::execute_with_protection(&env, &user, |env| {
        // Try to execute another protected function with same caller
        let nested_result = ReentryProtection::execute_with_protection(env, &user, |_env| {
            Ok(42i32)
        });
        
        // Should fail due to reentry
        assert_eq!(nested_result, Err(ReentryError::GuardAlreadyActive));
        
        Ok(100i32)
    });

    assert_eq!(result, Ok(100i32));
    assert!(!ReentryProtection::is_guard_active(&env, &user)); // Guard should be cleared
}
