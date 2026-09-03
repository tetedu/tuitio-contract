#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env, String};

fn setup() -> (Env, InstitutionRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(InstitutionRegistry, ());
    let client = InstitutionRegistryClient::new(&env, &id);
    client.initialize(&admin);
    (env, client, admin)
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
