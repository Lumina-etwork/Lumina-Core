#![no_std]

pub mod optimized;
pub mod benchmarks;
pub mod self_terminate;
pub mod multi_token;
pub mod yield_treasury;
pub mod yield_enhanced;
pub mod anti_reentry_guard;
pub mod virtual_accumulator;
pub mod authorized_lessor_registry;
pub mod fraud_clawback;

// Re-export optimized implementation
pub use optimized::{
    GrantContract, Grant, Error, DataKey,
    STATUS_ACTIVE, STATUS_PAUSED, STATUS_COMPLETED, STATUS_CANCELLED,
    STATUS_REVOCABLE, STATUS_MILESTONE_BASED, STATUS_AUTO_RENEW, STATUS_EMERGENCY_PAUSE,
    has_status, set_status, clear_status, toggle_status,
};

// Re-export self-termination implementation
pub use self_terminate::{
    GrantContract as SelfTerminateContract, SelfTerminateResult, SelfTerminateError,
    STATUS_SELF_TERMINATED, is_self_terminated, can_be_self_terminated,
    validate_self_terminate_transition,
};

// Re-export multi-token implementation
pub use multi_token::{
    GrantContract as MultiTokenContract, TokenBalance, TokenWithdrawal, MultiTokenWithdrawResult,
    MultiTokenGrant, MultiTokenError, create_token_balance, create_token_withdrawal,
};

// Re-export yield treasury implementation
pub use yield_treasury::{
    YieldTreasuryContract, YieldPosition, TreasuryConfig, YieldMetrics,
    YIELD_STATUS_INACTIVE, YIELD_STATUS_INVESTING, YIELD_STATUS_INVESTED, 
    YIELD_STATUS_DIVESTING, YIELD_STATUS_EMERGENCY,
    YIELD_STRATEGY_STELLAR_AQUA, YIELD_STRATEGY_STELLAR_USDC, YIELD_STRATEGY_LIQUIDITY_POOL,
    YieldError,
};

// Re-export yield-enhanced implementation
pub use yield_enhanced::{
    YieldEnhancedGrantContract, EnhancedGrant, EnhancedDataKey, EnhancedError,
};

// Re-export security implementations
pub use anti_reentry_guard::{
    AntiReentryContract, ReentryGuard, ReentryDataKey, ReentryProtection,
    ReentryError, REENTRY_GUARD_ACTIVE,
};

pub use virtual_accumulator::{
    VirtualAccumulatorContract, AccumulatorConfig, AccumulatorState, UserAccumulatorState,
    AccumulatorDataKey, AccumulatorError, VirtualAccumulator,
    PRECISION_MULTIPLIER, MAX_ACCUMULATION_PERIODS,
};

pub use authorized_lessor_registry::{
    AuthorizedLessorRegistryContract, AuthorizedLessor, InstitutionalData, LessorApproval,
    LessorRegistryDataKey, LessorRegistryError, LessorRegistry,
    LESSOR_STATUS_PENDING, LESSOR_STATUS_APPROVED, LESSOR_STATUS_SUSPENDED, LESSOR_STATUS_REVOKED,
    LESSOR_STATUS_INSTITUTIONAL, TIER_BASIC, TIER_STANDARD, TIER_PREMIUM, TIER_ENTERPRISE,
    has_lessor_status, set_lessor_status, clear_lessor_status,
};

pub use fraud_clawback::{
    FraudClawbackContract, FraudDispute, FraudResolution, JurorVote, SecurityCouncilMember,
    FraudDataKey, FraudError, FraudClawback,
    DISPUTE_STATUS_RAISED, DISPUTE_STATUS_FROZEN, DISPUTE_STATUS_JURY_SELECTED,
    DISPUTE_STATUS_VOTING, DISPUTE_STATUS_RESOLVED, DISPUTE_STATUS_DISMISSED,
    DISPUTE_STATUS_CONFIRMED_FRAUD, JURY_SIZE, VOTING_THRESHOLD, VOTING_PERIOD_DAYS,
    has_dispute_status, set_dispute_status,
};

#[cfg(test)]
pub use test_optimized::*;
#[cfg(test)]
pub use test_self_terminate::*;
#[cfg(test)]
pub use test_multi_token::*;
#[cfg(test)]
pub use test_yield::*;
#[cfg(test)]
mod test_security_precision;
