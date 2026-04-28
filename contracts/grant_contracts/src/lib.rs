#![no_std]

pub mod optimized;
// pub mod benchmarks;
// pub mod self_terminate;
// pub mod multi_token;
// pub mod yield_treasury;
// pub mod yield_enhanced;
// pub mod anti_reentry_guard;
// pub mod authorized_lessor_registry;
// pub mod virtual_accumulator;
// pub mod fraud_clawback;

// Re-export optimized implementation
pub use optimized::{
    GrantContract, Grant, Error, DataKey,
    STATUS_ACTIVE, STATUS_PAUSED, STATUS_COMPLETED, STATUS_CANCELLED,
    STATUS_REVOCABLE, STATUS_MILESTONE_BASED, STATUS_AUTO_RENEW, STATUS_EMERGENCY_PAUSE,
    has_status, set_status, clear_status, toggle_status,
};
