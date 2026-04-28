#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, Map,
};

#[contract]
pub struct AuthorizedLessorRegistry;

// Lessor status and compliance levels
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LessorStatus {
    Pending = 0,
    Active = 1,
    Suspended = 2,
    Revoked = 3,
    UnderReview = 4,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ComplianceLevel {
    Basic = 0,      // Basic KYC verification
    Standard = 1,   // Standard institutional verification
    Enhanced = 2,  // Enhanced due diligence
    Premium = 3,    // Full compliance with regulatory requirements
}

// Lessor information structure
#[derive(Clone)]
pub struct LessorInfo {
    pub lessor_address: Address,
    pub name: String,
    pub status: u32,
    pub compliance_level: u32,
    pub registration_date: u64,
    pub last_updated: u64,
    pub authorized_by: Address, // DAO or authorized entity
    pub max_total_allocation: i128,
    pub current_allocation: i128,
    pub jurisdiction: String,
    pub license_number: Option<String>,
    pub metadata: Map<String, String>, // Additional compliance data
}

// Vesting schedule for institutional lessors
#[derive(Clone)]
#[contracttype]
pub struct InstitutionalVestingSchedule {
    pub lessor_address: Address,
    pub grant_id: u64,
    pub total_amount: i128,
    pub vesting_start: u64,
    pub vesting_duration: u64, // in seconds
    pub cliff_duration: u64,   // cliff period before any vesting
    pub slice_period: u64,    // minimum period between vesting events
    pub created_at: u64,
    pub last_vesting_update: u64,
    pub vested_amount: i128,
    pub released_amount: i128,
    pub is_active: bool,
}

// Registry data keys
#[derive(Clone)]
#[contracttype]
pub enum RegistryDataKey {
    Admin,
    Lessor(Address),
    AllLessors,
    PendingApprovals,
    SuspendedLessors,
    InstitutionalSchedule(u64),
    LessorSchedules(Address),
    ComplianceReport,
    GlobalStats,
}

// Events and audit logs
#[derive(Clone)]
#[contracttype]
pub struct AuditLog {
    pub timestamp: u64,
    pub event_type: String,
    pub lessor_address: Address,
    pub performed_by: Address,
    pub details: String,
    pub previous_state: Option<String>,
    pub new_state: Option<String>,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum LessorRegistryError {
    NotAuthorized = 2000,
    LessorNotFound = 2001,
    LessorAlreadyExists = 2002,
    InvalidStatus = 2003,
    InvalidComplianceLevel = 2004,
    InsufficientAllocation = 2005,
    MaxAllocationExceeded = 2006,
    VestingScheduleNotFound = 2007,
    InvalidVestingParameters = 2008,
    ComplianceCheckFailed = 2009,
    JurisdictionNotSupported = 2010,
    LicenseRequired = 2011,
    AuditLogError = 2012,
}

// Supported jurisdictions and their requirements
const SUPPORTED_JURISDICTIONS: &[&str] = &[
    "US", "GB", "EU", "SG", "JP", "CH", "CA", "AU"
];

// Helper functions
fn read_admin(env: &Env) -> Result<Address, LessorRegistryError> {
    env.storage()
        .instance()
        .get(&RegistryDataKey::Admin)
        .ok_or(LessorRegistryError::NotAuthorized)
}

fn require_admin_auth(env: &Env) -> Result<(), LessorRegistryError> {
    let admin = read_admin(env)?;
    admin.require_auth();
    Ok(())
}

fn read_lessor(env: &Env, lessor_address: &Address) -> Result<LessorInfo, LessorRegistryError> {
    env.storage()
        .instance()
        .get(&RegistryDataKey::Lessor(lessor_address.clone()))
        .ok_or(LessorRegistryError::LessorNotFound)
}

fn write_lessor(env: &Env, lessor_info: &LessorInfo) {
    env.storage()
        .instance()
        .set(&RegistryDataKey::Lessor(lessor_info.lessor_address.clone()), lessor_info);
}

fn is_jurisdiction_supported(jurisdiction: &str) -> bool {
    SUPPORTED_JURISDICTIONS.contains(&jurisdiction)
}

fn validate_compliance_requirements(
    compliance_level: u32,
    jurisdiction: &str,
    license_number: Option<String>,
) -> Result<(), LessorRegistryError> {
    if !is_jurisdiction_supported(jurisdiction) {
        return Err(LessorRegistryError::JurisdictionNotSupported);
    }

    // Enhanced and Premium levels require license for certain jurisdictions
    if compliance_level >= 1 {
        match jurisdiction {
            "US" | "GB" | "EU" => {
                if license_number.is_none() {
                    return Err(LessorRegistryError::LicenseRequired);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn log_audit_event(
    env: &Env,
    event_type: String,
    lessor_address: Address,
    performed_by: Address,
    details: String,
    previous_state: Option<String>,
    new_state: Option<String>,
) {
    let audit_log = AuditLog {
        timestamp: env.ledger().timestamp(),
        event_type,
        lessor_address: lessor_address.clone(),
        performed_by,
        details,
        previous_state,
        new_state,
    };

    // In a real implementation, this would store audit logs
    // For now, we emit an event
    env.events().publish(
        (symbol_short!("audit"), lessor_address.clone()),
        (audit_log.timestamp, audit_log.event_type, audit_log.details),
    );
}

#[contractimpl]
impl AuthorizedLessorRegistry {
    /// Initialize the registry with admin
    pub fn initialize(env: Env, admin: Address) -> Result<(), LessorRegistryError> {
        if env.storage().instance().has(&RegistryDataKey::Admin) {
            return Err(LessorRegistryError::NotAuthorized);
        }

        admin.require_auth();
        env.storage().instance().set(&RegistryDataKey::Admin, &admin);
        
        // Initialize empty collections
        env.storage().instance().set(&RegistryDataKey::AllLessors, &Vec::<Address>::new(&env));
        env.storage().instance().set(&RegistryDataKey::PendingApprovals, &Vec::<Address>::new(&env));
        env.storage().instance().set(&RegistryDataKey::SuspendedLessors, &Vec::<Address>::new(&env));

        Ok(())
    }

    /// Register a new institutional lessor
    pub fn register_lessor(
        env: Env,
        lessor_address: Address,
        name: String,
        compliance_level: u32,
        jurisdiction: String,
        license_number: Option<String>,
        max_total_allocation: i128,
        metadata: Map<String, String>,
    ) -> Result<(), LessorRegistryError> {
        require_admin_auth(&env)?;

        // Check if lessor already exists
        if env.storage().instance().has(&RegistryDataKey::Lessor(lessor_address.clone())) {
            return Err(LessorRegistryError::LessorAlreadyExists);
        }

        // Validate compliance requirements
        validate_compliance_requirements(compliance_level, jurisdiction.as_str(), license_number)?;

        let now = env.ledger().timestamp();
        let admin = read_admin(&env)?;

        let lessor_info = LessorInfo {
            lessor_address: lessor_address.clone(),
            name: name.clone(),
            status: 0, // Pending
            compliance_level,
            registration_date: now,
            last_updated: now,
            authorized_by: admin,
            max_total_allocation,
            current_allocation: 0,
            jurisdiction: jurisdiction.clone(),
            license_number,
            metadata,
        };

        // Store lessor information
        write_lessor(&env, &lessor_info);

        // Add to pending approvals list
        let mut pending = env.storage()
            .instance()
            .get(&RegistryDataKey::PendingApprovals)
            .unwrap_or(Vec::<Address>::new(&env));
        pending.push_back(lessor_address.clone());
        env.storage().instance().set(&RegistryDataKey::PendingApprovals, &pending);

        // Log the registration event
        log_audit_event(
            &env,
            String::from_str(&env, "LESSOR_REGISTERED"),
            lessor_address.clone(),
            admin,
            format!("Lessor {} registered with compliance level {:?}", name, compliance_level),
            None,
            Some(String::from_str(&env, "PENDING")),
        );

        // Emit registration event
        env.events().publish(
            (symbol_short!("lessor_reg"), lessor_address.clone()),
            (name, compliance_level as u32, jurisdiction),
        );

        Ok(())
    }

    /// Approve a pending lessor registration
    pub fn approve_lessor(
        env: Env,
        lessor_address: Address,
    ) -> Result<(), LessorRegistryError> {
        require_admin_auth(&env)?;

        let mut lessor_info = read_lessor(&env, &lessor_address)?;
        
        if lessor_info.status != 0 {
            return Err(LessorRegistryError::InvalidStatus);
        }

        let previous_status = format!("{:?}", lessor_info.status);
        lessor_info.status = 1; // Active
        lessor_info.last_updated = env.ledger().timestamp();

        write_lessor(&env, &lessor_info);

        // Remove from pending approvals
        let mut pending = env.storage()
            .instance()
            .get(&RegistryDataKey::PendingApprovals)
            .unwrap_or(Vec::<Address>::new(&env));
        
        let index = pending.iter().position(|addr| addr == lessor_address);
        if let Some(idx) = index {
            pending.remove(idx.try_into().unwrap());
        }
        env.storage().instance().set(&RegistryDataKey::PendingApprovals, &pending);

        // Add to active lessors
        let mut all_lessors = env.storage()
            .instance()
            .get(&RegistryDataKey::AllLessors)
            .unwrap_or(Vec::<Address>::new(&env));
        all_lessors.push_back(lessor_address.clone());
        env.storage().instance().set(&RegistryDataKey::AllLessors, &all_lessors);

        // Log approval event
        log_audit_event(
            &env,
            String::from_str(&env, "LESSOR_APPROVED"),
            lessor_address.clone(),
            read_admin(&env)?,
            String::from_str(&env, "Lessor approved and activated"),
            Some(String::from_str(&env, &previous_status)),
            Some(String::from_str(&env, "ACTIVE")),
        );

        // Emit approval event
        env.events().publish(
            (symbol_short!("lessor_approved"), lessor_address.clone()),
            (lessor_info.name, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Create institutional vesting schedule
    pub fn create_institutional_vesting(
        env: Env,
        grant_id: u64,
        lessor_address: Address,
        total_amount: i128,
        vesting_duration: u64,
        cliff_duration: u64,
        slice_period: u64,
    ) -> Result<(), LessorRegistryError> {
        require_admin_auth(&env)?;

        let lessor_info = read_lessor(&env, &lessor_address)?;
        
        if lessor_info.status != 1 { // Active
            return Err(LessorRegistryError::InvalidStatus);
        }

        // Check allocation limits
        if lessor_info.current_allocation + total_amount > lessor_info.max_total_allocation {
            return Err(LessorRegistryError::MaxAllocationExceeded);
        }

        // Validate vesting parameters
        if total_amount <= 0 || vesting_duration == 0 || slice_period == 0 {
            return Err(LessorRegistryError::InvalidVestingParameters);
        }

        if cliff_duration > vesting_duration {
            return Err(LessorRegistryError::InvalidVestingParameters);
        }

        let now = env.ledger().timestamp();
        
        let vesting_schedule = InstitutionalVestingSchedule {
            lessor_address: lessor_address.clone(),
            grant_id,
            total_amount,
            vesting_start: now,
            vesting_duration,
            cliff_duration,
            slice_period,
            created_at: now,
            last_vesting_update: now,
            vested_amount: 0,
            released_amount: 0,
            is_active: true,
        };

        // Store vesting schedule
        env.storage().instance().set(
            &RegistryDataKey::InstitutionalSchedule(grant_id),
            &vesting_schedule,
        );

        // Update lessor's current allocation
        let mut updated_lessor = lessor_info.clone();
        updated_lessor.current_allocation += total_amount;
        updated_lessor.last_updated = now;
        write_lessor(&env, &updated_lessor);

        // Add to lessor's schedules
        let mut schedules = env.storage()
            .instance()
            .get(&RegistryDataKey::LessorSchedules(lessor_address.clone()))
            .unwrap_or(Vec::<u64>::new(&env));
        VirtualAccumulator::compact_accumulator(env.clone(), grant_id)?;
        schedules.push_back(grant_id);
        env.storage().instance().set(&RegistryDataKey::LessorSchedules(lessor_address), &schedules);

        // Log vesting creation
        log_audit_event(
            &env,
            String::from_str(&env, "VESTING_CREATED"),
            lessor_address.clone(),
            read_admin(&env)?,
            format!("Institutional vesting created for grant {}", grant_id),
            None,
            None,
        );

        // Emit vesting creation event
        env.events().publish(
            (symbol_short!("vesting_created"), grant_id),
            (lessor_address, total_amount, vesting_duration),
        );

        Ok(())
    }

    /// Suspend a lessor (emergency action)
    pub fn suspend_lessor(
        env: Env,
        lessor_address: Address,
        reason: String,
    ) -> Result<(), LessorRegistryError> {
        require_admin_auth(&env)?;

        let mut lessor_info = read_lessor(&env, &lessor_address)?;
        
        if lessor_info.status == 2 || lessor_info.status == 3 { // Suspended || Revoked
            return Err(LessorRegistryError::InvalidStatus);
        }

        let previous_status = format!("{:?}", lessor_info.status);
        lessor_info.status = 2; // Suspended
        lessor_info.last_updated = env.ledger().timestamp();

        write_lessor(&env, &lessor_info);

        // Add to suspended list
        let mut suspended = env.storage()
            .instance()
            .get(&RegistryDataKey::SuspendedLessors)
            .unwrap_or(Vec::<Address>::new(&env));
        suspended.push_back(lessor_address.clone());
        env.storage().instance().set(&RegistryDataKey::SuspendedLessors, &suspended);

        // Log suspension
        log_audit_event(
            &env,
            String::from_str(&env, "LESSOR_SUSPENDED"),
            lessor_address.clone(),
            read_admin(&env)?,
            reason.clone(),
            Some(String::from_str(&env, &previous_status)),
            Some(String::from_str(&env, "SUSPENDED")),
        );

        // Emit suspension event
        env.events().publish(
            (symbol_short!("lessor_suspended"), lessor_address.clone()),
            (reason, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Get lessor information
    pub fn get_lessor_info(env: Env, lessor_address: Address) -> Result<LessorInfo, LessorRegistryError> {
        read_lessor(&env, &lessor_address)
    }

    /// Check if lessor is authorized and active
    pub fn is_lessor_authorized(env: Env, lessor_address: Address) -> Result<bool, LessorRegistryError> {
        match read_lessor(&env, &lessor_address) {
            Ok(lessor_info) => Ok(lessor_info.status == 1), // Active
            Err(LessorRegistryError::LessorNotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get all active lessors
    pub fn get_active_lessors(env: Env) -> Result<Vec<Address>, LessorRegistryError> {
        Ok(env.storage()
            .instance()
            .get(&RegistryDataKey::AllLessors)
            .unwrap_or(Vec::<Address>::new(&env)))
    }

    /// Get pending approvals
    pub fn get_pending_approvals(env: Env) -> Result<Vec<Address>, LessorRegistryError> {
        Ok(env.storage()
            .instance()
            .get(&RegistryDataKey::PendingApprovals)
            .unwrap_or(Vec::<Address>::new(&env)))
    }

    /// Get institutional vesting schedule
    pub fn get_vesting_schedule(env: Env, grant_id: u64) -> Result<InstitutionalVestingSchedule, LessorRegistryError> {
        env.storage()
            .instance()
            .get(&RegistryDataKey::InstitutionalSchedule(grant_id))
            .ok_or(LessorRegistryError::VestingScheduleNotFound)
    }

    /// Update admin (only callable by current admin)
    pub fn update_admin(env: Env, new_admin: Address) -> Result<(), LessorRegistryError> {
        require_admin_auth(&env)?;
        new_admin.require_auth();
        
        let old_admin = read_admin(&env)?;
        env.storage().instance().set(&RegistryDataKey::Admin, &new_admin);

        // Log admin change
        log_audit_event(
            &env,
            String::from_str(&env, "ADMIN_UPDATED"),
            new_admin.clone(),
            old_admin,
            String::from_str(&env, "Registry admin updated"),
            None,
            None,
        );

        Ok(())
    }
}
