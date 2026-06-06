use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, String, Vec, U256};
use crate::{Vault, DataKey};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CompletedVestCertificate {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub original_vault_id: u64,
    pub completion_timestamp: u64,
    pub total_claimed: i128,
    pub total_assets: i128,
    pub asset_types: Vec<Address>,
    pub loyalty_score: u32, // 0-1000 (1000 = perfect loyalty)
    pub proof_of_work_verified: bool,
    pub certificate_id: U256,
    pub metadata_uri: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LoyaltyMetrics {
    pub total_vesting_duration: u64, // seconds
    pub actual_completion_time: u64, // seconds
    pub early_claims_count: u32,
    pub missed_milestones: u32,
    pub performance_cliffs_passed: u32,
    pub stake_duration: u64, // seconds staked if applicable
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WorkVerification {
    pub verified_by: Address, // Verifier contract or oracle
    pub verification_timestamp: u64,
    pub work_type: String, // "development", "research", "community", etc.
    pub impact_score: u32, // 0-100
    pub verification_data: String, // IPFS hash or similar
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CertificateQuery {
    pub beneficiary: Option<Address>,
    pub work_type: Option<String>,
    pub min_loyalty_score: Option<u32>,
    pub time_range_start: Option<u64>,
    pub time_range_end: Option<u64>,
    pub verified_only: Option<bool>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CertificateQueryResult {
    pub certificates: Vec<CompletedVestCertificate>,
    pub total_found: u64,
    pub page: u64,
    pub page_size: u64,
}

#[event]
pub struct CertificateIssued {
    #[topic]
    pub certificate_id: U256,
    #[topic]
    pub beneficiary: Address,
    pub vault_id: u64,
    pub completion_timestamp: u64,
    pub loyalty_score: u32,
}

#[event]
pub struct WorkVerified {
    #[topic]
    pub certificate_id: U256,
    #[topic]
    pub verified_by: Address,
    pub work_type: String,
    pub impact_score: u32,
}

#[contract]
pub struct VestingCertificateRegistry;

#[contractimpl]
impl VestingCertificateRegistry {
    /// Register a completed vest in the certificate registry
    /// This should be called when a vault fully completes its vesting period
    pub fn register_completed_vest(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
        original_vault: Vault,
        total_claimed: i128,
        total_assets: i128,
        asset_types: Vec<Address>,
        metadata_uri: Option<String>,
    ) -> U256 {
        // Only the vesting contract can register certificates
        let current_contract = env.current_contract_address();
        current_contract.require_auth();

        let completion_timestamp = env.ledger().timestamp();
        
        // Calculate loyalty score based on vesting behavior
        let loyalty_metrics = Self::calculate_loyalty_metrics(&env, vault_id, &original_vault);
        let loyalty_score = Self::calculate_loyalty_score(&loyalty_metrics);
        
        // Generate unique certificate ID
        let certificate_count = env.storage().instance().get::<_, u64>(&DataKey::CertificateCount).unwrap_or(0);
        let certificate_id = U256::from_u128(&env, (certificate_count + 1) as u128);
        
        // Create certificate
        let certificate = CompletedVestCertificate {
            vault_id,
            beneficiary: beneficiary.clone(),
            original_vault_id: vault_id,
            completion_timestamp,
            total_claimed,
            total_assets,
            asset_types: asset_types.clone(),
            loyalty_score,
            proof_of_work_verified: false, // Needs separate verification
            certificate_id: certificate_id.clone(),
            metadata_uri,
        };
        
        // Store certificate
        env.storage().instance().set(&DataKey::CertificateRegistry(certificate_id.clone()), &certificate);
        
        // Update beneficiary certificates list
        let mut beneficiary_certs = env.storage().instance()
            .get::<_, Vec<U256>>(&DataKey::BeneficiaryCertificates(beneficiary.clone()))
            .unwrap_or(Vec::new(&env));
        beneficiary_certs.push_back(certificate_id.clone());
        env.storage().instance().set(&DataKey::BeneficiaryCertificates(beneficiary.clone()), &beneficiary_certs);
        
        // Update indexes for efficient querying
        Self::update_indexes(&env, &certificate);
        
        // Update certificate count
        env.storage().instance().set(&DataKey::CertificateCount, &(certificate_count + 1));
        
        CertificateIssued {
            certificate_id: certificate_id.clone(),
            beneficiary: beneficiary.clone(),
            vault_id,
            completion_timestamp,
            loyalty_score,
        }.publish(&env);
        
        certificate_id
    }
    
    /// Verify proof of work for a certificate
    /// Called by authorized verifiers (oracles, job boards, etc.)
    pub fn verify_proof_of_work(
        env: Env,
        certificate_id: U256,
        work_type: String,
        impact_score: u32,
        verification_data: String,
    ) -> bool {
        // Check if caller is authorized verifier
        Self::require_verifier(&env);
        
        let mut certificate = Self::get_certificate(env.clone(), certificate_id.clone());
        
        if certificate.proof_of_work_verified {
            panic!("Certificate already verified");
        }
        
        // Create work verification record
        let verification = WorkVerification {
            verified_by: env.current_contract_address(),
            verification_timestamp: env.ledger().timestamp(),
            work_type: work_type.clone(),
            impact_score,
            verification_data,
        };
        
        // Store verification
        env.storage().instance().set(&DataKey::WorkVerification(certificate_id.clone()), &verification);
        
        // Update certificate
        certificate.proof_of_work_verified = true;
        env.storage().instance().set(&DataKey::CertificateRegistry(certificate_id.clone()), &certificate);
        
        // Update work type index
        let mut work_type_certs = env.storage().instance()
            .get::<_, Vec<U256>>(&DataKey::WorkTypeIndex(work_type.clone()))
            .unwrap_or(Vec::new(&env));
        work_type_certs.push_back(certificate_id.clone());
        env.storage().instance().set(&DataKey::WorkTypeIndex(work_type.clone()), &work_type_certs);
        
        WorkVerified {
            certificate_id: certificate_id.clone(),
            verified_by: env.current_contract_address(),
            work_type: work_type.clone(),
            impact_score,
        }.publish(&env);
        
        true
    }
    
    /// Query certificates based on various criteria
    pub fn query_certificates(
        env: Env,
        query: CertificateQuery,
        page: u64,
        page_size: u64,
    ) -> CertificateQueryResult {
        let mut certificates = Vec::new(&env);
        let mut all_certificates = Vec::new(&env);
        
        // Start with all certificates, then filter
        let certificate_count = env.storage().instance().get::<_, u64>(&DataKey::CertificateCount).unwrap_or(0);
        
        for i in 1..=certificate_count {
            let cert_id = U256::from_u128(&env, i as u128);
            if let Some(cert) = env.storage().instance().get::<_, CompletedVestCertificate>(&DataKey::CertificateRegistry(cert_id)) {
                all_certificates.push_back(cert);
            }
        }
        
        // Apply filters
        for cert in all_certificates.iter() {
            let mut matches = true;
            
            // Filter by beneficiary
            if let Some(beneficiary) = &query.beneficiary {
                if cert.beneficiary != *beneficiary {
                    matches = false;
                }
            }
            
            // Filter by verification status
            if let Some(verified_only) = query.verified_only {
                if verified_only && !cert.proof_of_work_verified {
                    matches = false;
                }
            }
            
            // Filter by loyalty score
            if let Some(min_score) = query.min_loyalty_score {
                if cert.loyalty_score < min_score {
                    matches = false;
                }
            }
            
            // Filter by time range
            if let Some(start) = query.time_range_start {
                if cert.completion_timestamp < start {
                    matches = false;
                }
            }
            if let Some(end) = query.time_range_end {
                if cert.completion_timestamp > end {
                    matches = false;
                }
            }
            
            // Filter by work type (requires checking verification)
            if let Some(work_type) = &query.work_type {
                if let Some(verification) = env.storage().instance().get::<_, WorkVerification>(&DataKey::WorkVerification(cert.certificate_id.clone())) {
                    if verification.work_type != *work_type {
                        matches = false;
                    }
                } else {
                    matches = false; // No verification means no work type
                }
            }
            
            if matches {
                certificates.push_back(cert.clone());
            }
        }
        
        let total_found = certificates.len();
        
        // Pagination
        let start_idx = (page * page_size) as usize;
        let end_idx = ((page + 1) * page_size) as usize;
        
        let paginated_certs = if start_idx < total_found as usize {
            let mut result = Vec::new(&env);
            for i in start_idx..end_idx.min(total_found as usize) {
                result.push_back(certificates.get(i as u32).unwrap().clone());
            }
            result
        } else {
            Vec::new(&env)
        };
        
        CertificateQueryResult {
            certificates: paginated_certs,
            total_found: total_found as u64,
            page,
            page_size,
        }
    }
    
    /// Get certificate by ID
    pub fn get_certificate(env: Env, certificate_id: U256) -> CompletedVestCertificate {
        env.storage().instance()
            .get::<_, CompletedVestCertificate>(&DataKey::CertificateRegistry(certificate_id))
            .expect("Certificate not found")
    }
    
    /// Get all certificates for a beneficiary
    pub fn get_beneficiary_certificates(env: Env, beneficiary: Address) -> Vec<U256> {
        env.storage().instance()
            .get::<_, Vec<U256>>(&DataKey::BeneficiaryCertificates(beneficiary))
            .unwrap_or(Vec::new(&env))
    }
    
    /// Get work verification for a certificate
    pub fn get_work_verification(env: Env, certificate_id: U256) -> Option<WorkVerification> {
        env.storage().instance()
            .get::<_, WorkVerification>(&DataKey::WorkVerification(certificate_id))
    }
    
    /// Set authorized verifier (admin only)
    pub fn set_verifier(env: Env, verifier: Address) {
        // This should be called by the vesting contract admin
        let current_contract = env.current_contract_address();
        current_contract.require_auth();
        
        env.storage().instance().set(&DataKey::CertificateVerifier, &verifier);
    }
    
    // --- Helper Functions ---
    
    fn calculate_loyalty_metrics(env: &Env, _vault_id: u64, vault: &Vault) -> LoyaltyMetrics {
        let total_duration = vault.end_time.saturating_sub(vault.start_time);
        let actual_completion_time = env.ledger().timestamp().saturating_sub(vault.start_time);
        
        // For now, use simplified metrics
        // In a full implementation, this would track:
        // - Early claims (claiming before full vesting)
        // - Missed milestones
        // - Performance cliffs passed
        // - Staking duration
        
        LoyaltyMetrics {
            total_vesting_duration: total_duration,
            actual_completion_time,
            early_claims_count: 0, // Would need to track this
            missed_milestones: 0,  // Would need to track this
            performance_cliffs_passed: 1, // Assume passed if completed
            stake_duration: 0, // Would need to track this
        }
    }
    
    fn calculate_loyalty_score(metrics: &LoyaltyMetrics) -> u32 {
        let mut score = 1000u32; // Start with perfect score
        
        // Deduct points for early completion (indicates impatience)
        if metrics.actual_completion_time < metrics.total_vesting_duration {
            let total_dur = metrics.total_vesting_duration as i128;
            let actual_time = metrics.actual_completion_time as i128;
            let early_diff = total_dur - actual_time;
            let deduction = (early_diff * 200) / total_dur;
            score = score.saturating_sub(deduction as u32);
        }
        
        // Deduct points for early claims
        score = score.saturating_sub(metrics.early_claims_count * 50);
        
        // Deduct points for missed milestones
        score = score.saturating_sub(metrics.missed_milestones * 100);
        
        // Add points for staking loyalty
        if metrics.stake_duration > 0 {
            let stake_dur = metrics.stake_duration as i128;
            let total_dur = metrics.total_vesting_duration as i128;
            let staking_bonus = (stake_dur * 100) / total_dur;
            score = score.saturating_add(staking_bonus as u32);
        }
        
        score.min(1000) // Cap at 1000
    }
    
    fn update_indexes(env: &Env, certificate: &CompletedVestCertificate) {
        // Update loyalty score index
        let loyalty_bucket = (certificate.loyalty_score / 100) * 100; // Group by 100s
        let mut loyalty_certs = env.storage().instance()
            .get::<_, Vec<U256>>(&DataKey::LoyaltyIndex(loyalty_bucket))
            .unwrap_or(Vec::new(env));
        loyalty_certs.push_back(certificate.certificate_id.clone());
        env.storage().instance().set(&DataKey::LoyaltyIndex(loyalty_bucket), &loyalty_certs);
        
        // Update completion time index (by year)
        let year = certificate.completion_timestamp / (365 * 24 * 60 * 60); // Unix timestamp to year
        let mut time_certs = env.storage().instance()
            .get::<_, Vec<U256>>(&DataKey::CompletionTimeIndex(year))
            .unwrap_or(Vec::new(env));
        time_certs.push_back(certificate.certificate_id.clone());

        env.storage().instance().set(&DataKey::CompletionTimeIndex(year), &time_certs);
    }
    
    fn require_verifier(env: &Env) {
        if let Some(verifier) = env.storage().instance().get::<_, Address>(&DataKey::CertificateVerifier) {
            verifier.require_auth();
        } else {
            // If no specific verifier set, require contract admin
            // This should be adapted based on the actual auth mechanism
            env.current_contract_address().require_auth();
        }
    }
}
