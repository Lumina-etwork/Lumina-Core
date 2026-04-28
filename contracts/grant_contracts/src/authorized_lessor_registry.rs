#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, Vec,
};

#[contract]
pub struct AuthorizedLessorRegistryContract;

// Registry status flags
pub const LESSOR_STATUS_PENDING: u32 = 0b00000001;
pub const LESSOR_STATUS_APPROVED: u32 = 0b00000010;
pub const LESSOR_STATUS_SUSPENDED: u32 = 0b00000100;
pub const LESSOR_STATUS_REVOKED: u32 = 0b00001000;
pub const LESSOR_STATUS_INSTITUTIONAL: u32 = 0b00010000;

// Institutional tiers
pub const TIER_BASIC: u32 = 1;
pub const TIER_STANDARD: u32 = 2;
pub const TIER_PREMIUM: u32 = 3;
pub const TIER_ENTERPRISE: u32 = 4;

#[derive(Clone)]
#[contracttype]
pub enum LessorRegistryDataKey {
    Admin,
    Lessor(Address),
    LessorList,
    Institution(Address),
    PendingApprovals,
}

#[derive(Clone)]
#[contracttype]
pub struct AuthorizedLessor {
    pub address: Address,
    pub name: String,
    pub status_mask: u32,
    pub tier: u32,
    pub max_total_allocation: i128,
    pub current_allocation: i128,
    pub registration_timestamp: u64,
    pub last_updated: u64,
    pub authorized_by: Address,
    pub compliance_score: u32, // 0-100
    pub kyc_verified: bool,
    pub institutional_data: Option<InstitutionalData>,
}

#[derive(Clone)]
#[contracttype]
pub struct InstitutionalData {
    pub institution_type: String, // "bank", "fund", "exchange", etc.
    pub registration_number: String,
    pub jurisdiction: String,
    pub regulatory_compliance: bool,
    pub audit_report_hash: Option<String>,
    pub risk_rating: u32, // 1-10
}

#[derive(Clone)]
#[contracttype]
pub struct LessorApproval {
    pub lessor_address: Address,
    pub requested_by: Address,
    pub approval_timestamp: u64,
    pub expires_at: u64,
    pub approved: bool,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum LessorRegistryError {
    NotInitialized = 3001,
    AlreadyInitialized = 3002,
    NotAuthorized = 3003,
    LessorNotFound = 3004,
    LessorAlreadyExists = 3005,
    InvalidStatus = 3006,
    InvalidTier = 3007,
    ExceedsAllocation = 3008,
    InsufficientCompliance = 3009,
    ApprovalExpired = 3010,
    AlreadyApproved = 3011,
    InvalidInstitutionalData = 3012,
}

// Registry helper functions
pub fn has_lessor_status(status_mask: u32, flag: u32) -> bool {
    (status_mask & flag) != 0
}

pub fn set_lessor_status(status_mask: u32, flag: u32) -> u32 {
    status_mask | flag
}

pub fn clear_lessor_status(status_mask: u32, flag: u32) -> u32 {
    status_mask & !flag
}

// Authorized Lessor Registry implementation
pub struct LessorRegistry;

impl LessorRegistry {
    // Initialize registry with admin
    pub fn initialize(env: &Env, admin: Address) -> Result<(), LessorRegistryError> {
        if env.storage()
            .instance()
            .get::<LessorRegistryDataKey, Address>(&LessorRegistryDataKey::Admin)
            .is_some()
        {
            return Err(LessorRegistryError::AlreadyInitialized);
        }

        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::Admin, &admin);

        // Initialize empty lists
        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::LessorList, &Vec::<Address>::new(&env));
        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::PendingApprovals, &Vec::<LessorApproval>::new(&env));

        Ok(())
    }

    // Read admin
    pub fn read_admin(env: &Env) -> Result<Address, LessorRegistryError> {
        env.storage()
            .instance()
            .get(&LessorRegistryDataKey::Admin)
            .ok_or(LessorRegistryError::NotInitialized)
    }

    // Require admin authorization
    pub fn require_admin_auth(env: &Env) -> Result<Address, LessorRegistryError> {
        let admin = Self::read_admin(env)?;
        admin.require_auth();
        Ok(admin)
    }

    // Register new lessor
    pub fn register_lessor(
        env: &Env,
        lessor_address: Address,
        name: String,
        tier: u32,
        max_allocation: i128,
        institutional_data: Option<InstitutionalData>,
    ) -> Result<(), LessorRegistryError> {
        // Check if lessor already exists
        if env.storage()
            .instance()
            .get::<LessorRegistryDataKey, AuthorizedLessor>(&LessorRegistryDataKey::Lessor(lessor_address.clone()))
            .is_some()
        {
            return Err(LessorRegistryError::LessorAlreadyExists);
        }

        // Validate tier
        if tier < TIER_BASIC || tier > TIER_ENTERPRISE {
            return Err(LessorRegistryError::InvalidTier);
        }

        // Validate institutional data if provided
        if let Some(ref data) = institutional_data {
            if data.institution_type.is_empty() || data.jurisdiction.is_empty() {
                return Err(LessorRegistryError::InvalidInstitutionalData);
            }
        }

        let current_time = env.ledger().timestamp();
        let admin = Self::read_admin(env)?;

        let mut status_mask = LESSOR_STATUS_PENDING;
        if institutional_data.is_some() {
            status_mask = set_lessor_status(status_mask, LESSOR_STATUS_INSTITUTIONAL);
        }

        let lessor = AuthorizedLessor {
            address: lessor_address.clone(),
            name,
            status_mask,
            tier,
            max_total_allocation: max_allocation,
            current_allocation: 0,
            registration_timestamp: current_time,
            last_updated: current_time,
            authorized_by: admin,
            compliance_score: 50, // Default compliance score
            kyc_verified: false,
            institutional_data,
        };

        // Store lessor
        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::Lessor(lessor_address), &lessor);

        // Add to lessor list
        let mut lessor_list: Vec<Address> = env.storage()
            .instance()
            .get(&LessorRegistryDataKey::LessorList)
            .unwrap_or_else(|| Vec::new(env));
        lessor_list.push_back(lessor_address);
        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::LessorList, &lessor_list);

        Ok(())
    }

    // Approve lessor
    pub fn approve_lessor(
        env: &Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        admin.require_auth();
        Self::require_admin_auth(env)?;

        let mut lessor = Self::read_lessor(env, &lessor_address)?;
        
        if has_lessor_status(lessor.status_mask, LESSOR_STATUS_APPROVED) {
            return Err(LessorRegistryError::AlreadyApproved);
        }

        lessor.status_mask = set_lessor_status(lessor.status_mask, LESSOR_STATUS_APPROVED);
        lessor.status_mask = clear_lessor_status(lessor.status_mask, LESSOR_STATUS_PENDING);
        lessor.last_updated = env.ledger().timestamp();
        lessor.authorized_by = admin;

        Self::write_lessor(env, &lessor_address, &lessor);
        Ok(())
    }

    // Suspend lessor
    pub fn suspend_lessor(
        env: &Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        admin.require_auth();
        Self::require_admin_auth(env)?;

        let mut lessor = Self::read_lessor(env, &lessor_address)?;
        
        lessor.status_mask = set_lessor_status(lessor.status_mask, LESSOR_STATUS_SUSPENDED);
        lessor.last_updated = env.ledger().timestamp();

        Self::write_lessor(env, &lessor_address, &lessor);
        Ok(())
    }

    // Revoke lessor
    pub fn revoke_lessor(
        env: &Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        admin.require_auth();
        Self::require_admin_auth(env)?;

        let mut lessor = Self::read_lessor(env, &lessor_address)?;
        
        lessor.status_mask = set_lessor_status(lessor.status_mask, LESSOR_STATUS_REVOKED);
        lessor.status_mask = clear_lessor_status(lessor.status_mask, LESSOR_STATUS_APPROVED);
        lessor.last_updated = env.ledger().timestamp();

        Self::write_lessor(env, &lessor_address, &lessor);
        Ok(())
    }

    // Update lessor allocation
    pub fn update_allocation(
        env: &Env,
        lessor_address: Address,
        new_allocation: i128,
    ) -> Result<(), LessorRegistryError> {
        lessor_address.require_auth();

        let mut lessor = Self::read_lessor(env, &lessor_address)?;
        
        if new_allocation > lessor.max_total_allocation {
            return Err(LessorRegistryError::ExceedsAllocation);
        }

        lessor.current_allocation = new_allocation;
        lessor.last_updated = env.ledger().timestamp();

        Self::write_lessor(env, &lessor_address, &lessor);
        Ok(())
    }

    // Check if lessor is authorized for operation
    pub fn is_lessor_authorized(env: &Env, lessor_address: &Address) -> Result<bool, LessorRegistryError> {
        let lessor = Self::read_lessor(env, lessor_address)?;
        
        let is_approved = has_lessor_status(lessor.status_mask, LESSOR_STATUS_APPROVED);
        let is_suspended = has_lessor_status(lessor.status_mask, LESSOR_STATUS_SUSPENDED);
        let is_revoked = has_lessor_status(lessor.status_mask, LESSOR_STATUS_REVOKED);

        Ok(is_approved && !is_suspended && !is_revoked)
    }

    // Read lessor
    pub fn read_lessor(env: &Env, lessor_address: &Address) -> Result<AuthorizedLessor, LessorRegistryError> {
        env.storage()
            .instance()
            .get(&LessorRegistryDataKey::Lessor(lessor_address.clone()))
            .ok_or(LessorRegistryError::LessorNotFound)
    }

    // Write lessor
    pub fn write_lessor(env: &Env, lessor_address: &Address, lessor: &AuthorizedLessor) {
        env.storage()
            .instance()
            .set(&LessorRegistryDataKey::Lessor(lessor_address.clone()), lessor);
    }

    // Get all lessors
    pub fn get_all_lessors(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&LessorRegistryDataKey::LessorList)
            .unwrap_or_else(|| Vec::new(env))
    }

    // Get authorized lessors only
    pub fn get_authorized_lessors(env: &Env) -> Vec<Address> {
        let all_lessors = Self::get_all_lessors(env);
        let mut authorized = Vec::new(env);

        for lessor_address in all_lessors {
            if let Ok(true) = Self::is_lessor_authorized(env, &lessor_address) {
                authorized.push_back(lessor_address);
            }
        }

        authorized
    }
}

#[contractimpl]
impl AuthorizedLessorRegistryContract {
    // Initialize registry
    pub fn initialize(env: Env, admin: Address) -> Result<(), LessorRegistryError> {
        LessorRegistry::initialize(&env, admin)
    }

    // Register new lessor
    pub fn register_lessor(
        env: Env,
        lessor_address: Address,
        name: String,
        tier: u32,
        max_allocation: i128,
        institutional_data: Option<InstitutionalData>,
    ) -> Result<(), LessorRegistryError> {
        LessorRegistry::register_lessor(&env, lessor_address, name, tier, max_allocation, institutional_data)
    }

    // Approve lessor
    pub fn approve_lessor(
        env: Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        LessorRegistry::approve_lessor(&env, admin, lessor_address)
    }

    // Suspend lessor
    pub fn suspend_lessor(
        env: Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        LessorRegistry::suspend_lessor(&env, admin, lessor_address)
    }

    // Revoke lessor
    pub fn revoke_lessor(
        env: Env,
        admin: Address,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        LessorRegistry::revoke_lessor(&env, admin, lessor_address)
    }

    // Update allocation
    pub fn update_allocation(
        env: Env,
        lessor_address: Address,
        new_allocation: i128,
    ) -> Result<(), LessorRegistryError> {
        LessorRegistry::update_allocation(&env, lessor_address, new_allocation)
    }

    // Check authorization
    pub fn is_authorized(env: Env, lessor_address: Address) -> Result<bool, LessorRegistryError> {
        LessorRegistry::is_lessor_authorized(&env, &lessor_address)
    }

    // Get lessor info
    pub fn get_lessor(env: Env, lessor_address: Address) -> Result<AuthorizedLessor, LessorRegistryError> {
        LessorRegistry::read_lessor(&env, &lessor_address)
    }

    // Get all lessors
    pub fn get_all_lessors(env: Env) -> Vec<Address> {
        LessorRegistry::get_all_lessors(&env)
    }

    // Get authorized lessors
    pub fn get_authorized_lessors(env: Env) -> Vec<Address> {
        LessorRegistry::get_authorized_lessors(&env)
    }
}
