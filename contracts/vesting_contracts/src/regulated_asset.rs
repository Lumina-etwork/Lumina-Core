#![no_std]
use soroban_sdk::{
    contracttype,
    contracterror,
    Address,
    Env,
    Vec,
    String,
    Symbol,
    BytesN,
};

/// SEP-08 regulated asset errors
#[contracterror]
#[repr(u32)]
pub enum RegulatedAssetError {
    AssetNotRegulated = 1,
    AuthorizationRequired = 2,
    AuthorizationRevoked = 3,
    AssetFrozen = 4,
    ClawbackExecuted = 5,
    InvalidAuthorization = 6,
    AuthorizationExpired = 7,
    InsufficientAuthorization = 8,
    AssetNotSupported = 9,
    ComplianceCheckFailed = 10,
}

/// SEP-08 authorization status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AuthorizationStatus {
    None,
    Pending,
    Active,
    Revoked,
    Expired,
    Frozen,
}

/// SEP-08 authorization data
#[derive(Clone)]
#[contracttype]
pub struct SEP08Authorization {
    pub asset_id: Address,
    pub holder: Address,
    pub authorized_amount: i128,
    pub used_amount: i128,
    pub authorization_id: BytesN<32>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub issuer: Address,
    pub status: AuthorizationStatus,
    pub compliance_flags: u32,
}

/// Asset regulation metadata
#[derive(Clone)]
#[contracttype]
pub struct AssetRegulation {
    pub asset_id: Address,
    pub is_regulated: bool,
    pub requires_authorization: bool,
    pub supports_freeze: bool,
    pub supports_clawback: bool,
    pub max_authorization_duration: u64,
    pub issuer: Address,
    pub regulation_version: u32,
    pub compliance_requirements: Vec<String>,
}

/// Freeze/clawback event data
#[derive(Clone)]
#[contracttype]
pub struct FreezeEvent {
    pub asset_id: Address,
    pub holder: Address,
    pub amount: i128,
    pub reason: String,
    pub timestamp: u64,
    pub issuer_signature: BytesN<32>,
}

/// Clawback event data
#[derive(Clone)]
#[contracttype]
pub struct ClawbackEvent {
    pub asset_id: Address,
    pub from_holder: Address,
    pub amount: i128,
    pub reason: String,
    pub timestamp: u64,
    pub issuer_signature: BytesN<32>,
}

/// Storage keys for regulated assets
pub const ASSET_REGULATIONS: Bytes = Bytes::from_short_bytes("ASSET_REGULATIONS");
pub const SEP08_AUTHORIZATIONS: Bytes = Bytes::from_short_bytes("SEP08_AUTHORIZATIONS");
pub const FREEZE_EVENTS: Bytes = Bytes::from_short_bytes("FREEZE_EVENTS");
pub const CLAWBACK_EVENTS: Bytes = Bytes::from_short_bytes("CLAWBACK_EVENTS");

/// SEP-08 Regulated Asset Manager
pub struct RegulatedAssetManager;

impl RegulatedAssetManager {
    /// Register a regulated asset with the system
    pub fn register_regulated_asset(
        env: &Env,
        asset_id: Address,
        issuer: Address,
        requires_authorization: bool,
        supports_freeze: bool,
        supports_clawback: bool,
        max_authorization_duration: u64,
        compliance_requirements: Vec<String>,
    ) -> Result<(), RegulatedAssetError> {
        // Check if asset already registered
        let reg_key = (ASSET_REGULATIONS, asset_id.clone());
        if env.storage().persistent().has(&reg_key) {
            return Err(RegulatedAssetError::AssetNotSupported);
        }

        let regulation = AssetRegulation {
            asset_id: asset_id.clone(),
            is_regulated: true,
            requires_authorization,
            supports_freeze,
            supports_clawback,
            max_authorization_duration,
            issuer: issuer.clone(),
            regulation_version: 1,
            compliance_requirements,
        };

        env.storage().persistent().set(&reg_key, &regulation);

        // Emit registration event
        AssetRegistered {
            asset_id,
            issuer,
            requires_authorization,
            supports_freeze,
            supports_clawback,
        }.publish(env);

        Ok(())
    }

    /// Check if asset requires SEP-08 authorization
    pub fn requires_authorization(env: &Env, asset_id: Address) -> bool {
        let reg_key = (ASSET_REGULATIONS, asset_id);
        if let Some(regulation) = env.storage().persistent().get::<_, AssetRegulation>(&reg_key) {
            regulation.requires_authorization
        } else {
            false
        }
    }

    /// Check if asset supports freeze operations
    pub fn supports_freeze(env: &Env, asset_id: Address) -> bool {
        let reg_key = (ASSET_REGULATIONS, asset_id);
        if let Some(regulation) = env.storage().persistent().get::<_, AssetRegulation>(&reg_key) {
            regulation.supports_freeze
        } else {
            false
        }
    }

    /// Check if asset supports clawback operations
    pub fn supports_clawback(env: &Env, asset_id: Address) -> bool {
        let reg_key = (ASSET_REGULATIONS, asset_id);
        if let Some(regulation) = env.storage().persistent().get::<_, AssetRegulation>(&reg_key) {
            regulation.supports_clawback
        } else {
            false
        }
    }

    /// Create SEP-08 authorization for regulated asset
    pub fn create_authorization(
        env: &Env,
        asset_id: Address,
        holder: Address,
        authorized_amount: i128,
        authorization_id: BytesN<32>,
        expires_at: u64,
        issuer: Address,
        compliance_flags: u32,
    ) -> Result<(), RegulatedAssetError> {
        // Verify asset is regulated and requires authorization
        if !Self::requires_authorization(env, asset_id.clone()) {
            return Err(RegulatedAssetError::AssetNotRegulated);
        }

        // Verify issuer is authorized for this asset
        let reg_key = (ASSET_REGULATIONS, asset_id.clone());
        let regulation: AssetRegulation = env.storage().persistent()
            .get(&reg_key)
            .ok_or(RegulatedAssetError::AssetNotRegulated)?;

        if regulation.issuer != issuer {
            return Err(RegulatedAssetError::InvalidAuthorization);
        }

        // Check authorization doesn't already exist
        let auth_key = (SEP08_AUTHORIZATIONS, authorization_id.clone());
        if env.storage().persistent().has(&auth_key) {
            return Err(RegulatedAssetError::InvalidAuthorization);
        }

        let authorization = SEP08Authorization {
            asset_id: asset_id.clone(),
            holder: holder.clone(),
            authorized_amount,
            used_amount: 0,
            authorization_id: authorization_id.clone(),
            issued_at: env.ledger().timestamp(),
            expires_at,
            issuer: issuer.clone(),
            status: AuthorizationStatus::Active,
            compliance_flags,
        };

        env.storage().persistent().set(&auth_key, &authorization);

        // Emit authorization event
        AuthorizationCreated {
            asset_id,
            holder,
            authorized_amount,
            authorization_id,
            expires_at,
            issuer,
        }.publish(env);

        Ok(())
    }

    /// Validate SEP-08 authorization for transfer
    pub fn validate_authorization(
        env: &Env,
        asset_id: Address,
        holder: Address,
        amount: i128,
        authorization_id: BytesN<32>,
    ) -> Result<(), RegulatedAssetError> {
        // Get authorization
        let auth_key = (SEP08_AUTHORIZATIONS, authorization_id.clone());
        let authorization: SEP08Authorization = env.storage().persistent()
            .get(&auth_key)
            .ok_or(RegulatedAssetError::AuthorizationRequired)?;

        // Verify authorization matches asset and holder
        if authorization.asset_id != asset_id || authorization.holder != holder {
            return Err(RegulatedAssetError::InvalidAuthorization);
        }

        // Check authorization status
        match authorization.status {
            AuthorizationStatus::Active => {
                // Check if expired
                if env.ledger().timestamp() > authorization.expires_at {
                    return Err(RegulatedAssetError::AuthorizationExpired);
                }

                // Check sufficient authorized amount
                let available = authorization.authorized_amount - authorization.used_amount;
                if amount > available {
                    return Err(RegulatedAssetError::InsufficientAuthorization);
                }

                Ok(())
            }
            AuthorizationStatus::Revoked => Err(RegulatedAssetError::AuthorizationRevoked),
            AuthorizationStatus::Expired => Err(RegulatedAssetError::AuthorizationExpired),
            AuthorizationStatus::Frozen => Err(RegulatedAssetError::AssetFrozen),
            _ => Err(RegulatedAssetError::InvalidAuthorization),
        }
    }

    /// Consume authorization amount (after successful transfer)
    pub fn consume_authorization(
        env: &Env,
        authorization_id: BytesN<32>,
        amount: i128,
    ) {
        let auth_key = (SEP08_AUTHORIZATIONS, authorization_id.clone());
        if let Some(mut authorization) = env.storage().persistent().get::<_, SEP08Authorization>(&auth_key) {
            authorization.used_amount += amount;
            env.storage().persistent().set(&auth_key, &authorization);

            // Emit consumption event
            AuthorizationConsumed {
                authorization_id,
                amount,
                remaining: authorization.authorized_amount - authorization.used_amount,
            }.publish(env);
        }
    }

    /// Handle asset freeze event from issuer
    pub fn handle_freeze_event(
        env: &Env,
        asset_id: Address,
        holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError> {
        // Verify asset supports freeze
        if !Self::supports_freeze(env, asset_id.clone()) {
            return Err(RegulatedAssetError::AssetNotSupported);
        }

        let freeze_event = FreezeEvent {
            asset_id: asset_id.clone(),
            holder: holder.clone(),
            amount,
            reason: reason.clone(),
            timestamp: env.ledger().timestamp(),
            issuer_signature,
        };

        // Store freeze event
        let freeze_key = (FREEZE_EVENTS, asset_id.clone(), holder.clone(), env.ledger().timestamp());
        env.storage().persistent().set(&freeze_key, &freeze_event);

        // Update all active authorizations for this holder/asset to frozen
        Self::update_authorizations_status(env, asset_id, holder, AuthorizationStatus::Frozen);

        // Emit freeze event
        AssetFrozen {
            asset_id,
            holder,
            amount,
            reason,
        }.publish(env);

        Ok(())
    }

    /// Handle asset clawback event from issuer
    pub fn handle_clawback_event(
        env: &Env,
        asset_id: Address,
        from_holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError> {
        // Verify asset supports clawback
        if !Self::supports_clawback(env, asset_id.clone()) {
            return Err(RegulatedAssetError::AssetNotSupported);
        }

        let clawback_event = ClawbackEvent {
            asset_id: asset_id.clone(),
            from_holder: from_holder.clone(),
            amount,
            reason: reason.clone(),
            timestamp: env.ledger().timestamp(),
            issuer_signature,
        };

        // Store clawback event
        let clawback_key = (CLAWBACK_EVENTS, asset_id.clone(), from_holder.clone(), env.ledger().timestamp());
        env.storage().persistent().set(&clawback_key, &clawback_event);

        // Revoke all active authorizations for this holder/asset
        Self::update_authorizations_status(env, asset_id, from_holder, AuthorizationStatus::Revoked);

        // Emit clawback event
        AssetClawback {
            asset_id,
            from_holder,
            amount,
            reason,
        }.publish(env);

        Ok(())
    }

    /// Revoke specific authorization
    pub fn revoke_authorization(
        env: &Env,
        issuer: Address,
        authorization_id: BytesN<32>,
        reason: String,
    ) -> Result<(), RegulatedAssetError> {
        let auth_key = (SEP08_AUTHORIZATIONS, authorization_id.clone());
        let mut authorization: SEP08Authorization = env.storage().persistent()
            .get(&auth_key)
            .ok_or(RegulatedAssetError::InvalidAuthorization)?;

        // Verify issuer
        if authorization.issuer != issuer {
            return Err(RegulatedAssetError::InvalidAuthorization);
        }

        // Update status
        authorization.status = AuthorizationStatus::Revoked;
        env.storage().persistent().set(&auth_key, &authorization);

        // Emit revocation event
        AuthorizationRevoked {
            authorization_id,
            issuer,
            reason,
        }.publish(env);

        Ok(())
    }

    /// Get asset regulation info
    pub fn get_asset_regulation(env: &Env, asset_id: Address) -> Option<AssetRegulation> {
        let reg_key = (ASSET_REGULATIONS, asset_id);
        env.storage().persistent().get(&reg_key)
    }

    /// Get authorization by ID
    pub fn get_authorization(env: &Env, authorization_id: BytesN<32>) -> Option<SEP08Authorization> {
        let auth_key = (SEP08_AUTHORIZATIONS, authorization_id);
        env.storage().persistent().get(&auth_key)
    }

    /// Get all authorizations for holder
    pub fn get_holder_authorizations(env: &Env, holder: Address) -> Vec<SEP08Authorization> {
        // This is a simplified implementation
        // In production, you'd want an index for efficient queries
        Vec::new(env)
    }

    /// Check if holder has any frozen assets
    pub fn has_frozen_assets(env: &Env, holder: Address) -> bool {
        // Simplified implementation - in production, check freeze events
        false
    }

    /// Get compliance requirements for asset
    pub fn get_compliance_requirements(env: &Env, asset_id: Address) -> Vec<String> {
        if let Some(regulation) = Self::get_asset_regulation(env, asset_id) {
            regulation.compliance_requirements
        } else {
            Vec::new(env)
        }
    }

    // Private helper methods

    fn update_authorizations_status(
        env: &Env,
        asset_id: Address,
        holder: Address,
        new_status: AuthorizationStatus,
    ) {
        // This is a simplified implementation
        // In production, you'd iterate through all authorizations and update matching ones
        // For now, we'll rely on the validation function to check status
    }
}

/// Events for SEP-08 regulated assets
#[event]
pub struct AssetRegistered {
    #[topic]
    pub asset_id: Address,
    #[topic]
    pub issuer: Address,
    pub requires_authorization: bool,
    pub supports_freeze: bool,
    pub supports_clawback: bool,
}

#[event]
pub struct AuthorizationCreated {
    #[topic]
    pub asset_id: Address,
    #[topic]
    pub holder: Address,
    pub authorized_amount: i128,
    #[topic]
    pub authorization_id: BytesN<32>,
    pub expires_at: u64,
    #[topic]
    pub issuer: Address,
}

#[event]
pub struct AuthorizationConsumed {
    #[topic]
    pub authorization_id: BytesN<32>,
    pub amount: i128,
    pub remaining: i128,
}

#[event]
pub struct AuthorizationRevoked {
    #[topic]
    pub authorization_id: BytesN<32>,
    #[topic]
    pub issuer: Address,
    pub reason: String,
}

#[event]
pub struct AssetFrozen {
    #[topic]
    pub asset_id: Address,
    #[topic]
    pub holder: Address,
    pub amount: i128,
    pub reason: String,
}

#[event]
pub struct AssetClawback {
    #[topic]
    pub asset_id: Address,
    #[topic]
    pub from_holder: Address,
    pub amount: i128,
    pub reason: String,
}

/// Public interface for SEP-08 regulated assets
pub trait RegulatedAssetTrait {
    /// Register regulated asset
    fn register_regulated_asset(
        env: Env,
        asset_id: Address,
        issuer: Address,
        requires_authorization: bool,
        supports_freeze: bool,
        supports_clawback: bool,
        max_authorization_duration: u64,
        compliance_requirements: Vec<String>,
    ) -> Result<(), RegulatedAssetError>;

    /// Create authorization
    fn create_authorization(
        env: Env,
        asset_id: Address,
        holder: Address,
        authorized_amount: i128,
        authorization_id: BytesN<32>,
        expires_at: u64,
        issuer: Address,
        compliance_flags: u32,
    ) -> Result<(), RegulatedAssetError>;

    /// Validate authorization
    fn validate_authorization(
        env: Env,
        asset_id: Address,
        holder: Address,
        amount: i128,
        authorization_id: BytesN<32>,
    ) -> Result<(), RegulatedAssetError>;

    /// Handle freeze event
    fn handle_freeze_event(
        env: Env,
        asset_id: Address,
        holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError>;

    /// Handle clawback event
    fn handle_clawback_event(
        env: Env,
        asset_id: Address,
        from_holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError>;

    /// Get asset regulation
    fn get_asset_regulation(env: Env, asset_id: Address) -> Option<AssetRegulation>;

    /// Get authorization
    fn get_authorization(env: Env, authorization_id: BytesN<32>) -> Option<SEP08Authorization>;
}
