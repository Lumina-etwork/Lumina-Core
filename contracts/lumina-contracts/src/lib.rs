#![no_std]

//! Lumina Network core contract.
//!
//! Two responsibilities, both of which surface as Soroban contract events that
//! the off-chain `Lumina-Backend` indexer consumes byte-for-byte:
//!
//! * `anchor_ip` records a creator's SHA-256 IP fingerprint (`BytesN<32>`) and
//!   publishes `ip_anchor(creator)` with `(asset_id, fingerprint, anchored_at)`.
//! * `create_escrow` / `release_milestone` run a client-funded milestone
//!   escrow and publish `escrow_new(client)` with
//!   `(escrow_id, creator, total_amount, total_milestones)` and
//!   `pay_rel(escrow_id)` with `(payout, completed_milestones)`.
//!
//! Event names must stay in lockstep with `src/indexer/scval.js` in the backend.
//! Note `symbol_short!` only accepts up to 9 characters, which is why
//! `escrow_new` (10) is built with `Symbol::new` while the others are not.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol,
};

/// One ledger is closed roughly every five seconds.
const DAY_IN_LEDGERS: u32 = 17_280;
const ESCROW_TTL: u32 = 30 * DAY_IN_LEDGERS;
const ESCROW_TTL_THRESHOLD: u32 = ESCROW_TTL - DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    EscrowCount,
    Escrow(u64),
    Asset(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub client: Address,
    pub creator: Address,
    pub total_amount: i128,
    pub remaining_balance: i128,
    pub total_milestones: u32,
    pub completed_milestones: u32,
    pub settled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpAsset {
    pub creator: Address,
    pub fingerprint: BytesN<32>,
    pub anchored_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidMilestones = 4,
    EscrowNotFound = 5,
    EscrowSettled = 6,
    Unauthorized = 7,
    ArithmeticOverflow = 8,
}

#[contract]
pub struct LuminaContract;

#[contractimpl]
impl LuminaContract {
    /// Bind the contract to an admin. Callable exactly once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::EscrowCount, &0u64);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    /// Anchor `fingerprint` as the creator's proof of authorship for `asset_id`.
    pub fn anchor_ip(
        env: Env,
        creator: Address,
        asset_id: u64,
        fingerprint: BytesN<32>,
    ) -> Result<(), Error> {
        require_initialized(&env)?;
        creator.require_auth();

        let anchored_at = env.ledger().timestamp();
        let asset = IpAsset {
            creator: creator.clone(),
            fingerprint: fingerprint.clone(),
            anchored_at,
        };

        let key = DataKey::Asset(asset_id);
        env.storage().persistent().set(&key, &asset);
        env.storage()
            .persistent()
            .extend_ttl(&key, ESCROW_TTL_THRESHOLD, ESCROW_TTL);

        env.events().publish(
            (symbol_short!("ip_anchor"), creator),
            (asset_id, fingerprint, anchored_at),
        );
        Ok(())
    }

    pub fn get_asset(env: Env, asset_id: u64) -> Option<IpAsset> {
        env.storage().persistent().get(&DataKey::Asset(asset_id))
    }

    /// Create a client-funded escrow split across `total_milestones` releases.
    /// Returns the freshly assigned escrow id.
    pub fn create_escrow(
        env: Env,
        client: Address,
        creator: Address,
        total_amount: i128,
        total_milestones: u32,
    ) -> Result<u64, Error> {
        require_initialized(&env)?;
        client.require_auth();

        if total_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if total_milestones == 0 {
            return Err(Error::InvalidMilestones);
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0);
        let escrow_id = count.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        env.storage().instance().set(&DataKey::EscrowCount, &escrow_id);

        let escrow = Escrow {
            client: client.clone(),
            creator: creator.clone(),
            total_amount,
            remaining_balance: total_amount,
            total_milestones,
            completed_milestones: 0,
            settled: false,
        };
        write_escrow(&env, escrow_id, &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_new"), client),
            (escrow_id, creator, total_amount, total_milestones),
        );
        Ok(escrow_id)
    }

    /// Release the next milestone. Only the funding client may call this.
    /// Returns `(payout, completed_milestones)`.
    pub fn release_milestone(
        env: Env,
        client: Address,
        escrow_id: u64,
    ) -> Result<(i128, u32), Error> {
        client.require_auth();

        let mut escrow = read_escrow(&env, escrow_id)?;
        if escrow.settled {
            return Err(Error::EscrowSettled);
        }
        if escrow.client != client {
            return Err(Error::Unauthorized);
        }

        let completed = escrow
            .completed_milestones
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;

        // Even split, except the final release sweeps whatever remains so the
        // escrow always settles to exactly zero without rounding dust.
        let payout = if completed >= escrow.total_milestones {
            escrow.remaining_balance
        } else {
            escrow.total_amount / (escrow.total_milestones as i128)
        };

        escrow.completed_milestones = completed;
        escrow.remaining_balance = escrow
            .remaining_balance
            .checked_sub(payout)
            .ok_or(Error::ArithmeticOverflow)?;
        if completed >= escrow.total_milestones {
            escrow.settled = true;
        }
        write_escrow(&env, escrow_id, &escrow);

        env.events()
            .publish((symbol_short!("pay_rel"), escrow_id), (payout, completed));

        Ok((payout, completed))
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    }

    pub fn escrow_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0)
    }
}

fn require_initialized(env: &Env) -> Result<(), Error> {
    if env.storage().instance().has(&DataKey::Admin) {
        Ok(())
    } else {
        Err(Error::NotInitialized)
    }
}

fn write_escrow(env: &Env, escrow_id: u64, escrow: &Escrow) {
    let key = DataKey::Escrow(escrow_id);
    env.storage().persistent().set(&key, escrow);
    env.storage()
        .persistent()
        .extend_ttl(&key, ESCROW_TTL_THRESHOLD, ESCROW_TTL);
}

fn read_escrow(env: &Env, escrow_id: u64) -> Result<Escrow, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Escrow(escrow_id))
        .ok_or(Error::EscrowNotFound)
}

#[cfg(test)]
mod test;
