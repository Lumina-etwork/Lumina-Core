use crate::{Error, LuminaContract, LuminaContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, BytesN, Env, Symbol, TryFromVal, TryIntoVal, Val, Vec,
};

/// Builds a fresh `Env` + initialised contract client in the caller's scope.
/// Returning both from a helper would be a self-referential borrow, so the
/// setup is expanded where the client is actually used.
macro_rules! setup {
    ($env:ident, $client:ident) => {
        let $env = Env::default();
        $env.mock_all_auths();
        let contract_id = $env.register(LuminaContract, ());
        let $client = LuminaContractClient::new(&$env, &contract_id);
    };
}

/// Returns `(event_name, topics, data)` for the most recently emitted event.
fn last_event(env: &Env) -> (Symbol, Vec<Val>, Val) {
    let events = env.events().all();
    assert!(events.len() > 0, "expected at least one event");
    let (_, topics, data) = events.get(events.len() - 1).unwrap();
    let name = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
    (name, topics, data)
}

fn topic_address(env: &Env, topics: &Vec<Val>, index: u32) -> Address {
    Address::try_from_val(env, &topics.get(index).unwrap()).unwrap()
}

#[test]
fn initialize_is_one_shot() {
    setup!(env, client);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert_eq!(client.admin(), admin);
    assert_eq!(client.try_initialize(&admin), Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn anchor_ip_stores_fingerprint_and_emits_event() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    let fingerprint = BytesN::from_array(&env, &[7u8; 32]);
    client.anchor_ip(&creator, &42, &fingerprint);

    let (name, topics, data) = last_event(&env);
    assert_eq!(name, symbol_short!("ip_anchor"));
    assert_eq!(topic_address(&env, &topics, 1), creator);

    let (asset_id, emitted_fp, _timestamp): (u64, BytesN<32>, u64) =
        TryFromVal::try_from_val(&env, &data).unwrap();
    assert_eq!(asset_id, 42);
    assert_eq!(emitted_fp, fingerprint);

    let stored = client.get_asset(&42).unwrap();
    assert_eq!(stored.creator, creator);
    assert_eq!(stored.fingerprint, fingerprint);
}

#[test]
fn anchor_ip_requires_initialization() {
    setup!(env, client);
    let creator = Address::generate(&env);
    let fingerprint = BytesN::from_array(&env, &[1u8; 32]);

    assert_eq!(
        client.try_anchor_ip(&creator, &1, &fingerprint),
        Err(Ok(Error::NotInitialized)),
    );
}

#[test]
fn create_escrow_assigns_ids_and_emits_escrow_new() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    let escrow_id = client.create_escrow(&funder, &creator, &1000i128, &2u32);
    assert_eq!(escrow_id, 1);

    let (name, topics, data) = last_event(&env);
    assert_eq!(name, Symbol::new(&env, "escrow_new"));
    assert_eq!(topic_address(&env, &topics, 1), funder);

    assert_eq!(client.escrow_count(), 1);

    let (id, emitter, total, milestones): (u64, Address, i128, u32) =
        TryFromVal::try_from_val(&env, &data).unwrap();
    assert_eq!(id, 1);
    assert_eq!(emitter, creator);
    assert_eq!(total, 1000);
    assert_eq!(milestones, 2);

    let stored = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(stored.remaining_balance, 1000);
    assert_eq!(stored.completed_milestones, 0);
    assert!(!stored.settled);
}

#[test]
fn rejects_zero_amount_and_zero_milestones() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client.try_create_escrow(&funder, &creator, &0i128, &2u32),
        Err(Ok(Error::InvalidAmount)),
    );
    assert_eq!(
        client.try_create_escrow(&funder, &creator, &100i128, &0u32),
        Err(Ok(Error::InvalidMilestones)),
    );
}

#[test]
fn release_milestone_emits_pay_rel_and_settles() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);
    let escrow_id = client.create_escrow(&funder, &creator, &1000i128, &2u32);

    let (payout, completed) = client.release_milestone(&funder, &escrow_id);
    assert_eq!((payout, completed), (500i128, 1u32));

    let (name, topics, data) = last_event(&env);
    assert_eq!(name, symbol_short!("pay_rel"));
    assert_eq!(
        u64::try_from_val(&env, &topics.get(1).unwrap()).unwrap(),
        escrow_id,
    );
    let (emitted_payout, emitted_completed): (i128, u32) =
        TryFromVal::try_from_val(&env, &data).unwrap();
    assert_eq!((emitted_payout, emitted_completed), (500i128, 1u32));

    let (final_payout, final_completed) = client.release_milestone(&funder, &escrow_id);
    assert_eq!((final_payout, final_completed), (500i128, 2u32));

    let stored = client.get_escrow(&escrow_id).unwrap();
    assert!(stored.settled);
    assert_eq!(stored.completed_milestones, 2);
    assert_eq!(stored.remaining_balance, 0);
}

#[test]
fn final_milestone_sweeps_rounding_remainder() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    let escrow_id = client.create_escrow(&funder, &creator, &1000i128, &3u32);
    assert_eq!(client.release_milestone(&funder, &escrow_id).0, 333);
    assert_eq!(client.release_milestone(&funder, &escrow_id).0, 333);
    assert_eq!(client.release_milestone(&funder, &escrow_id).0, 334);

    let stored = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(stored.remaining_balance, 0);
    assert!(stored.settled);
}

#[test]
fn only_the_funder_may_release() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.initialize(&admin);
    let escrow_id = client.create_escrow(&funder, &creator, &1000i128, &2u32);

    assert_eq!(
        client.try_release_milestone(&stranger, &escrow_id),
        Err(Ok(Error::Unauthorized)),
    );
}

#[test]
fn cannot_release_unknown_or_settled_escrow() {
    setup!(env, client);
    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client.try_release_milestone(&funder, &999u64),
        Err(Ok(Error::EscrowNotFound)),
    );

    let escrow_id = client.create_escrow(&funder, &creator, &100i128, &1u32);
    client.release_milestone(&funder, &escrow_id);
    assert_eq!(
        client.try_release_milestone(&funder, &escrow_id),
        Err(Ok(Error::EscrowSettled)),
    );
}

#[test]
fn event_data_wire_shape_is_a_positional_vec() {
    // Guards against a tuple being emitted as an ScMap: the backend decoder
    // assumes `value` is an ScVec and indexes it positionally.
    setup!(env, client);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let creator = Address::generate(&env);
    client.anchor_ip(&creator, &7, &BytesN::from_array(&env, &[9u8; 32]));

    let (_, _, data) = last_event(&env);
    let as_vec: Vec<Val> = data.try_into_val(&env).unwrap();
    assert_eq!(as_vec.len(), 3);
}
