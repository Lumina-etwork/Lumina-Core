#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, U256};

mod errors;
pub use errors::Error;

#[contract]
pub struct GrantContract;

const TOTAL_AMOUNT: Symbol = symbol_short!("TOTAL");
const START_TIME: Symbol = symbol_short!("START");
const END_TIME: Symbol = symbol_short!("END");
const RECIPIENT: Symbol = symbol_short!("RECIPIENT");
const CLAIMED: Symbol = symbol_short!("CLAIMED");

// 10 years in seconds (Issue #44)
const MAX_DURATION: u64 = 315_360_000;

#[contractimpl]
impl GrantContract {
    pub fn initialize_grant(
        env: Env,
        recipient: Address,
        total_amount: U256,
        duration_seconds: u64,
    ) -> u64 {
        assert!(
            duration_seconds <= MAX_DURATION,
            "duration exceeds MAX_DURATION"
        );
        let start_time = env.ledger().timestamp();
        let end_time = start_time + duration_seconds;

        env.storage().instance().set(&TOTAL_AMOUNT, &total_amount);
        env.storage().instance().set(&START_TIME, &start_time);
        env.storage().instance().set(&END_TIME, &end_time);
        env.storage().instance().set(&RECIPIENT, &recipient);
        env.storage()
            .instance()
            .set(&CLAIMED, &U256::from_u32(&env, 0));
        end_time
    }

    pub fn claimable_balance(env: Env) -> U256 {
        let current_time = env.ledger().timestamp();
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let total_amount = env
            .storage()
            .instance()
            .get(&TOTAL_AMOUNT)
            .unwrap_or(U256::from_u32(&env, 0));
        let claimed = env
            .storage()
            .instance()
            .get(&CLAIMED)
            .unwrap_or(U256::from_u32(&env, 0));
        if current_time <= start_time {
            return U256::from_u32(&env, 0);
        }

        let elapsed = if current_time >= end_time {
            end_time - start_time
        } else {
            current_time - start_time
        };

        let total_duration = end_time - start_time;
        let vested = if total_duration > 0 {
            let elapsed_u256 = U256::from_u32(&env, elapsed as u32);
            let duration_u256 = U256::from_u32(&env, total_duration as u32);
            total_amount.mul(&elapsed_u256).div(&duration_u256)
        } else {
            U256::from_u32(&env, 0)
        };

        if vested > claimed {
            vested.sub(&claimed)
        } else {
            U256::from_u32(&env, 0)
        }
    }

    pub fn claim(env: Env, recipient: Address) -> Result<U256, Error> {
        recipient.require_auth();

        // ========== COMPLIANCE CHECKS ==========

        // KYC Verification Check
        if !Self::is_kyc_verified(&env, &recipient) {
            return Err(Error::KycNotCompleted);
        }

        // KYC Expiration Check
        if let Some(kyc_expiry) = Self::get_kyc_expiry(&env, &recipient) {
            let current_time = env.ledger().timestamp();
            if current_time > kyc_expiry {
                return Err(Error::KycExpired);
            }
        }

        // Sanctions Check
        if Self::is_address_sanctioned(&env, &recipient) {
            return Err(Error::AddressSanctioned);
        }

        // Jurisdiction Restriction Check
        if Self::is_jurisdiction_restricted(&env, &recipient) {
            return Err(Error::JurisdictionRestricted);
        }

        // Legal Signature Verification
        if !Self::has_valid_legal_signature(&env, &recipient) {
            return Err(Error::LegalSignatureMissing);
        }

        // Document Verification Check
        if !Self::are_documents_verified(&env, &recipient) {
            return Err(Error::DocumentVerificationFailed);
        }

        // Tax Compliance Check
        if !Self::is_tax_compliant(&env, &recipient) {
            return Err(Error::TaxComplianceFailed);
        }

        // Whitelist Approval Check
        if !Self::is_whitelist_approved(&env, &recipient) {
            return Err(Error::WhitelistNotApproved);
        }

        // Blacklist Violation Check
        if Self::is_on_blacklist(&env, &recipient) {
            return Err(Error::BlacklistViolation);
        }

        // Geofencing Restriction Check
        if Self::is_geofencing_restricted(&env, &recipient) {
            return Err(Error::GeofencingRestriction);
        }

        // Identity Verification Expiration Check
        if let Some(identity_expiry) = Self::get_identity_expiry(&env, &recipient) {
            let current_time = env.ledger().timestamp();
            if current_time > identity_expiry {
                return Err(Error::IdentityVerificationExpired);
            }
        }

        // Politically Exposed Person Check
        if Self::is_politically_exposed_person(&env, &recipient) {
            return Err(Error::PoliticallyExposedPerson);
        }

        // Sanctions List Hit Check
        if Self::is_on_sanctions_list(&env, &recipient) {
            return Err(Error::SanctionsListHit);
        }

        let stored_recipient = env.storage().instance().get(&RECIPIENT).unwrap();
        if recipient != stored_recipient {
            return Err(Error::Unauthorized);
        }

        let claimable = Self::claimable_balance(env.clone());
        if claimable <= U256::from_u32(&env, 0) {
            return Err(Error::NothingToClaim);
        }

        let claimed = env
            .storage()
            .instance()
            .get(&CLAIMED)
            .unwrap_or(U256::from_u32(&env, 0));
        let new_claimed = claimed.add(&claimable);
        env.storage().instance().set(&CLAIMED, &new_claimed);

        Ok(claimable)
    }

    pub fn get_grant_info(env: Env) -> (U256, u64, u64, U256) {
        let total_amount = env
            .storage()
            .instance()
            .get(&TOTAL_AMOUNT)
            .unwrap_or(U256::from_u32(&env, 0));
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let claimed = env
            .storage()
            .instance()
            .get(&CLAIMED)
            .unwrap_or(U256::from_u32(&env, 0));
        (total_amount, start_time, end_time, claimed)
    }

    // ========== COMPLIANCE HELPER FUNCTIONS ==========

    /// Check if user has completed KYC verification
    fn is_kyc_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual KYC verification check
        // This would typically integrate with a KYC provider oracle
        // For now, return true as placeholder
        true
    }

    /// Get KYC expiration timestamp for user
    fn get_kyc_expiry(_e: &Env, _user: &Address) -> Option<u64> {
        // TODO: Implement actual KYC expiry check
        // This would typically be stored from KYC provider data
        // For now, return None (no expiry)
        None
    }

    /// Check if address is on sanctions list
    fn is_address_sanctioned(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual sanctions check
        // This would integrate with sanctions screening oracle
        // For now, return false as placeholder
        false
    }

    /// Check if user's jurisdiction is restricted
    fn is_jurisdiction_restricted(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual jurisdiction check
        // This would check user's location against restricted jurisdictions
        // For now, return false as placeholder
        false
    }

    /// Check if user has valid legal signature
    fn has_valid_legal_signature(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual legal signature verification
        // This would verify digital signatures against legal documents
        // For now, return true as placeholder
        true
    }

    /// Check if user's documents are verified
    fn are_documents_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual document verification check
        // This would check verification status of required documents
        // For now, return true as placeholder
        true
    }

    /// Check if user is tax compliant
    fn is_tax_compliant(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual tax compliance check
        // This would check tax withholding and reporting status
        // For now, return true as placeholder
        true
    }

    /// Check if user is approved on whitelist
    fn is_whitelist_approved(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual whitelist approval check
        // This would check against approved investor whitelist
        // For now, return true as placeholder
        true
    }

    /// Check if user is on blacklist
    fn is_on_blacklist(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual blacklist check
        // This would check against prohibited persons list
        // For now, return false as placeholder
        false
    }

    /// Check if user is subject to geofencing restrictions
    fn is_geofencing_restricted(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual geofencing check
        // This would check IP/location-based restrictions
        // For now, return false as placeholder
        false
    }

    /// Get identity verification expiration for user
    fn get_identity_expiry(_e: &Env, _user: &Address) -> Option<u64> {
        // TODO: Implement actual identity expiry check
        // This would check when identity verification expires
        // For now, return None (no expiry)
        None
    }

    /// Check if user is a politically exposed person
    fn is_politically_exposed_person(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual PEP check
        // This would screen against PEP lists
        // For now, return false as placeholder
        false
    }

    /// Check if user appears on sanctions lists
    fn is_on_sanctions_list(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual sanctions list screening
        // This would check multiple sanctions databases
        // For now, return false as placeholder
        false
    }
}

mod test;
