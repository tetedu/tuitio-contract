#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Env, String};

fn setup() -> (Env, InstitutionRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(InstitutionRegistry, ());
    let client = InstitutionRegistryClient::new(&env, &id);
    client.initialize(&admin);
    (env, client, admin)
}

/// Registers a fresh institution and returns its address.
fn register_named(
    env: &Env,
    client: &InstitutionRegistryClient,
    name: &str,
    country: &str,
) -> Address {
    let school = Address::generate(env);
    let payout = Address::generate(env);
    client.register(
        &school,
        &payout,
        &String::from_str(env, name),
        &String::from_str(env, country),
    );
    school
}

fn register_school(env: &Env, client: &InstitutionRegistryClient) -> (Address, Address) {
    let school = Address::generate(env);
    let payout = Address::generate(env);
    client.register(
        &school,
        &payout,
        &String::from_str(env, "Kigali Technical College"),
        &String::from_str(env, "RW"),
    );
    (school, payout)
}

#[test]
fn initialize_sets_admin_and_zero_count() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.count(), 0);
}

#[test]
fn initialize_is_single_shot() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    let err = client.try_initialize(&other).err().unwrap().unwrap();
    assert_eq!(err, RegistryError::AlreadyInitialized);
}

#[test]
fn register_creates_pending_record() {
    let (env, client, _admin) = setup();
    let (school, payout) = register_school(&env, &client);

    let record = client.get_institution(&school);
    assert_eq!(record.status, InstitutionStatus::Pending);
    assert_eq!(record.payout, payout);
    assert_eq!(record.admin, school);
    assert_eq!(client.count(), 1);
    // Pending institutions must not be payable.
    assert!(!client.is_verified(&school));
}

#[test]
fn register_rejects_duplicate_address() {
    let (env, client, _admin) = setup();
    let (school, payout) = register_school(&env, &client);
    let err = client
        .try_register(
            &school,
            &payout,
            &String::from_str(&env, "Second Attempt"),
            &String::from_str(&env, "RW"),
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, RegistryError::InstitutionExists);
}

#[test]
fn register_rejects_bad_metadata() {
    let (env, client, _admin) = setup();
    let school = Address::generate(&env);
    let payout = Address::generate(&env);

    // Empty name.
    let err = client
        .try_register(
            &school,
            &payout,
            &String::from_str(&env, ""),
            &String::from_str(&env, "RW"),
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, RegistryError::InvalidMetadata);

    // Country code that is not ISO alpha-2.
    let err = client
        .try_register(
            &school,
            &payout,
            &String::from_str(&env, "Valid Name"),
            &String::from_str(&env, "RWA"),
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, RegistryError::InvalidMetadata);
}

#[test]
fn verify_makes_institution_payable() {
    let (env, client, _admin) = setup();
    let (school, payout) = register_school(&env, &client);

    client.verify(&school);

    assert!(client.is_verified(&school));
    assert_eq!(client.payout_of(&school), payout);
    assert_eq!(
        client.get_institution(&school).status,
        InstitutionStatus::Verified
    );
}

#[test]
fn verify_rejects_already_verified() {
    let (env, client, _admin) = setup();
    let (school, _payout) = register_school(&env, &client);
    client.verify(&school);
    let err = client.try_verify(&school).err().unwrap().unwrap();
    assert_eq!(err, RegistryError::InvalidStatus);
}

#[test]
fn verify_rejects_unknown_institution() {
    let (env, client, _admin) = setup();
    let ghost = Address::generate(&env);
    let err = client.try_verify(&ghost).err().unwrap().unwrap();
    assert_eq!(err, RegistryError::InstitutionNotFound);
}

#[test]
fn suspend_blocks_payout_lookup() {
    let (env, client, _admin) = setup();
    let (school, _payout) = register_school(&env, &client);
    client.verify(&school);

    client.suspend(&school);

    assert!(!client.is_verified(&school));
    let err = client.try_payout_of(&school).err().unwrap().unwrap();
    assert_eq!(err, RegistryError::InvalidStatus);
}

#[test]
fn suspend_requires_verified_state() {
    let (env, client, _admin) = setup();
    let (school, _payout) = register_school(&env, &client);
    // Still Pending, never verified.
    let err = client.try_suspend(&school).err().unwrap().unwrap();
    assert_eq!(err, RegistryError::InvalidStatus);
}

#[test]
fn suspended_institution_can_be_reinstated() {
    let (env, client, _admin) = setup();
    let (school, _payout) = register_school(&env, &client);
    client.verify(&school);
    client.suspend(&school);

    client.verify(&school);

    assert!(client.is_verified(&school));
}

#[test]
fn update_payout_rotates_address() {
    let (env, client, _admin) = setup();
    let (school, _payout) = register_school(&env, &client);
    client.verify(&school);
    let new_payout = Address::generate(&env);

    client.update_payout(&school, &new_payout);

    assert_eq!(client.payout_of(&school), new_payout);
}

#[test]
fn is_verified_is_false_for_unknown_address() {
    let (env, client, _admin) = setup();
    let ghost = Address::generate(&env);
    assert!(!client.is_verified(&ghost));
}

#[test]
fn set_admin_transfers_control() {
    let (env, client, _admin) = setup();
    let new_admin = Address::generate(&env);

    client.set_admin(&new_admin);

    assert_eq!(client.admin(), new_admin);
}

#[test]
fn count_tracks_registrations() {
    let (env, client, _admin) = setup();
    register_school(&env, &client);
    register_school(&env, &client);
    register_school(&env, &client);
    assert_eq!(client.count(), 3);
}

// ----- paginated listing ----------------------------------------------------

#[test]
fn list_returns_registration_order() {
    let (env, client, _admin) = setup();
    let first = register_named(&env, &client, "First College", "RW");
    let second = register_named(&env, &client, "Second Institute", "NG");
    let third = register_named(&env, &client, "Third Academy", "KE");

    let page = client.list(&0, &10);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap(), first);
    assert_eq!(page.get(1).unwrap(), second);
    assert_eq!(page.get(2).unwrap(), third);
}

#[test]
fn list_pages_through_the_registry() {
    let (env, client, _admin) = setup();
    let a = register_named(&env, &client, "A", "RW");
    let b = register_named(&env, &client, "B", "RW");
    let c = register_named(&env, &client, "C", "RW");

    let first = client.list(&0, &2);
    assert_eq!(first.len(), 2);
    assert_eq!(first.get(0).unwrap(), a);
    assert_eq!(first.get(1).unwrap(), b);

    let second = client.list(&2, &2);
    assert_eq!(second.len(), 1, "last page is short");
    assert_eq!(second.get(0).unwrap(), c);
}

#[test]
fn list_past_the_end_is_empty_not_an_error() {
    let (env, client, _admin) = setup();
    register_named(&env, &client, "Only", "RW");
    assert_eq!(client.list(&1, &10).len(), 0);
    assert_eq!(client.list(&99, &10).len(), 0);
}

#[test]
fn list_on_an_empty_registry_is_empty() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.list(&0, &10).len(), 0);
}

#[test]
fn list_rejects_a_zero_limit() {
    let (env, client, _admin) = setup();
    register_named(&env, &client, "Only", "RW");
    assert_eq!(client.list(&0, &0).len(), 0);
}

#[test]
fn list_clamps_an_oversized_limit() {
    let (env, client, _admin) = setup();
    register_named(&env, &client, "A", "RW");
    register_named(&env, &client, "B", "RW");

    // Asking for more than MAX_PAGE must not error; it returns what exists.
    let page = client.list(&0, &(MAX_PAGE + 5_000));
    assert_eq!(page.len(), 2);
}

#[test]
fn list_includes_unverified_and_suspended_institutions() {
    let (env, client, _admin) = setup();
    let pending = register_named(&env, &client, "Pending School", "RW");
    let verified = register_named(&env, &client, "Verified School", "RW");
    client.verify(&verified);
    let suspended = register_named(&env, &client, "Suspended School", "RW");
    client.verify(&suspended);
    client.suspend(&suspended);

    // list is an index of the registry, not a filter on status; callers decide
    // what to do with each record.
    let page = client.list(&0, &10);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap(), pending);
    assert_eq!(page.get(1).unwrap(), verified);
    assert_eq!(page.get(2).unwrap(), suspended);
}

#[test]
fn list_agrees_with_count() {
    let (env, client, _admin) = setup();
    for _ in 0..5 {
        register_named(&env, &client, "School", "RW");
    }
    assert_eq!(client.count(), 5);
    assert_eq!(client.list(&0, &100).len(), client.count());
}

#[test]
fn initialize_emits_a_genesis_event() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(InstitutionRegistry, ());
    let client = InstitutionRegistryClient::new(&env, &id);

    client.initialize(&admin);

    // The registry's configuration must be recoverable from the event log
    // alone, so an indexer never has to be told the admin out of band.
    assert_eq!(
        env.events().all().events().len(),
        1,
        "initialize should publish exactly one genesis event"
    );
}
