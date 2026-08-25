#![doc = include_str!("../CORE.md")]
#![cfg_attr(not(test), no_std)]
extern crate alloc;
pub mod core;
pub mod pool;
pub mod identity;
pub mod net;
pub mod attestation;
pub mod db_migration;
pub mod cache;
