use soroban_sdk::{Env, Address, Vec, Map};
use crate::types::*;

#[derive(Clone)]
pub enum DataKey {
    Admin,
    SecurityCouncil,
    SupportedAssets,
    AuthorizedAdapters,
    TimelockDuration,
    IsInitialized,
    Balance(Address),
    BailoutRequest(u64),
    NextRequestId,
}

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn require_admin(e: &Env, admin: &Address) {
    if &get_admin(e) != admin {
        panic!("Unauthorized");
    }
}

pub fn set_security_council(e: &Env, council: &Vec<Address>) {
    e.storage().instance().set(&DataKey::SecurityCouncil, council);
}

pub fn get_security_council(e: &Env) -> Vec<Address> {
    e.storage().instance().get(&DataKey::SecurityCouncil).unwrap_or(Vec::new(e))
}

pub fn set_supported_assets(e: &Env, assets: &Vec<Address>) {
    e.storage().instance().set(&DataKey::SupportedAssets, assets);
}

pub fn get_supported_assets(e: &Env) -> Vec<Address> {
    e.storage().instance().get(&DataKey::SupportedAssets).unwrap_or(Vec::new(e))
}

pub fn set_authorized_adapters(e: &Env, adapters: &Vec<Address>) {
    e.storage().instance().set(&DataKey::AuthorizedAdapters, adapters);
}

pub fn get_authorized_adapters(e: &Env) -> Vec<Address> {
    e.storage().instance().get(&DataKey::AuthorizedAdapters).unwrap_or(Vec::new(e))
}

pub fn set_timelock_duration(e: &Env, duration: u64) {
    e.storage().instance().set(&DataKey::TimelockDuration, &duration);
}

pub fn get_timelock_duration(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::TimelockDuration).unwrap_or(14 * 24 * 60 * 60)
}

pub fn set_is_initialized(e: &Env, initialized: bool) {
    e.storage().instance().set(&DataKey::IsInitialized, &initialized);
}

pub fn is_initialized(e: &Env) -> bool {
    e.storage().instance().get(&DataKey::IsInitialized).unwrap_or(false)
}

pub fn set_balance(e: &Env, asset: &Address, balance: i128) {
    e.storage().instance().set(&DataKey::Balance(asset.clone()), &balance);
}

pub fn get_balance(e: &Env, asset: &Address) -> i128 {
    e.storage().instance().get(&DataKey::Balance(asset.clone())).unwrap_or(0)
}

pub fn set_bailout_request(e: &Env, id: u64, request: &BailoutRequest) {
    e.storage().instance().set(&DataKey::BailoutRequest(id), request);
}

pub fn get_bailout_request(e: &Env, id: u64) -> BailoutRequest {
    e.storage().instance().get(&DataKey::BailoutRequest(id)).unwrap()
}

pub fn set_next_request_id(e: &Env, id: u64) {
    e.storage().instance().set(&DataKey::NextRequestId, &id);
}

pub fn get_next_request_id(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::NextRequestId).unwrap_or(0)
}