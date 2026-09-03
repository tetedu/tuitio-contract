#![cfg(test)]

use super::*;
use institution_registry::{InstitutionRegistry, InstitutionRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, String,
};

/// One academic term, priced in USDC's 7 decimals. 50.0000000 USDC.
const TERM_AMOUNT: i128 = 50_0000000;
const TERMS: u32 = 4;
const WINDOW: u64 = 7 * 24 * 60 * 60; // 7 days
const SPONSOR_FUNDING: i128 = 10_000_0000000;

struct Fixture<'a> {
    env: Env,
    escrow: TuitionEscrowClient<'a>,
    registry: InstitutionRegistryClient<'a>,
    token: token::TokenClient<'a>,
    sponsor: Address,
    beneficiary: Address,
    institution: Address,
    school_payout: Address,
    escrow_admin: Address,
}

fn setup() -> Fixture<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let registry_admin = Address::generate(&env);
    let escrow_admin = Address::generate(&env);
    let sponsor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let institution = Address::generate(&env);
    let school_payout = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let registry_id = env.register(InstitutionRegistry, ());
    let registry = InstitutionRegistryClient::new(&env, &registry_id);
    registry.initialize(&registry_admin);

    let escrow_id = env.register(TuitionEscrow, ());
    let escrow = TuitionEscrowClient::new(&env, &escrow_id);
    escrow.initialize(&escrow_admin, &registry_id, &WINDOW);

    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_id = sac.address();
    token::StellarAssetClient::new(&env, &token_id).mint(&sponsor, &SPONSOR_FUNDING);

    registry.register(
        &institution,
        &school_payout,
        &String::from_str(&env, "Kigali Technical College"),
        &String::from_str(&env, "RW"),
    );
    registry.verify(&institution);

    Fixture {
        env: env.clone(),
        escrow,
        registry,
        token: token::TokenClient::new(&env, &token_id),
        sponsor,
        beneficiary,
        institution,
        school_payout,
        escrow_admin,
    }
}

impl Fixture<'_> {
    fn create(&self) -> u64 {
        self.escrow.create_grant(
            &self.sponsor,
            &self.beneficiary,
            &self.institution,
            &self.token.address,
            &TERM_AMOUNT,
            &TERMS,
        )
    }

    fn advance(&self, seconds: u64) {
        let now = self.env.ledger().timestamp();
        self.env.ledger().with_mut(|l| l.timestamp = now + seconds);
    }

    /// Attest a term and let its dispute window elapse.
    fn attest_and_wait(&self, grant_id: u64, term: u32) {
        self.escrow.attest_term(&grant_id, &term);
        self.advance(WINDOW + 1);
    }
}

// ----- creation -------------------------------------------------------------

#[test]
fn create_grant_pulls_full_commitment_into_escrow() {
    let f = setup();
    let grant_id = f.create();

    let total = TERM_AMOUNT * i128::from(TERMS);
    assert_eq!(f.token.balance(&f.escrow.address), total);
    assert_eq!(f.token.balance(&f.sponsor), SPONSOR_FUNDING - total);
    assert_eq!(f.escrow.locked_amount(&grant_id), total);

    let grant = f.escrow.get_grant(&grant_id);
    assert_eq!(grant.status, GrantStatus::Active);
    assert_eq!(grant.next_term, 0);
    assert_eq!(grant.terms_total, TERMS);
    assert_eq!(grant.beneficiary, f.beneficiary);
}

#[test]
fn create_grant_rejects_unverified_institution() {
    let f = setup();
    let unverified = Address::generate(&f.env);
    f.registry.register(
        &unverified,
        &Address::generate(&f.env),
        &String::from_str(&f.env, "Unvetted Academy"),
        &String::from_str(&f.env, "NG"),
    );
    // Registered but still Pending.
    let err = f
        .escrow
        .try_create_grant(
            &f.sponsor,
            &f.beneficiary,
            &unverified,
            &f.token.address,
            &TERM_AMOUNT,
            &TERMS,
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::InstitutionNotVerified);
}

#[test]
fn create_grant_rejects_unknown_institution() {
    let f = setup();
    let ghost = Address::generate(&f.env);
    let err = f
        .escrow
        .try_create_grant(
            &f.sponsor,
            &f.beneficiary,
            &ghost,
            &f.token.address,
            &TERM_AMOUNT,
            &TERMS,
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::InstitutionNotVerified);
}

#[test]
fn create_grant_rejects_bad_economics() {
    let f = setup();
    for (amount, terms, expected) in [
        (0i128, TERMS, EscrowError::InvalidAmount),
        (-1i128, TERMS, EscrowError::InvalidAmount),
        (TERM_AMOUNT, 0u32, EscrowError::InvalidTermCount),
    ] {
        let err = f
            .escrow
            .try_create_grant(
                &f.sponsor,
                &f.beneficiary,
                &f.institution,
                &f.token.address,
                &amount,
                &terms,
            )
            .err()
            .unwrap()
            .unwrap();
        assert_eq!(err, expected);
    }
}

#[test]
fn create_grant_rejects_overflowing_total() {
    let f = setup();
    let err = f
        .escrow
        .try_create_grant(
            &f.sponsor,
            &f.beneficiary,
            &f.institution,
            &f.token.address,
            &(i128::MAX / 2),
            &4u32,
        )
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::Overflow);
}

#[test]
fn grant_ids_increment() {
    let f = setup();
    assert_eq!(f.create(), 0);
    assert_eq!(f.create(), 1);
    assert_eq!(f.escrow.next_grant_id(), 2);
}

// ----- attestation ----------------------------------------------------------

#[test]
fn attest_starts_window_without_moving_funds() {
    let f = setup();
    let grant_id = f.create();
    let escrow_balance = f.token.balance(&f.escrow.address);

    f.escrow.attest_term(&grant_id, &0);

    // This is the core safety property of attestation: it is a claim, not a payment.
    assert_eq!(f.token.balance(&f.escrow.address), escrow_balance);
    assert_eq!(f.token.balance(&f.school_payout), 0);

    let term = f.escrow.get_term(&grant_id, &0);
    assert_eq!(term.status, TermStatus::Attested);
    assert_eq!(term.release_after, term.attested_at + WINDOW);
}

#[test]
fn attest_must_follow_term_order() {
    let f = setup();
    let grant_id = f.create();
    let err = f
        .escrow
        .try_attest_term(&grant_id, &2)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::OutOfOrderTerm);
}

#[test]
fn attest_rejects_term_beyond_grant() {
    let f = setup();
    let grant_id = f.create();
    let err = f
        .escrow
        .try_attest_term(&grant_id, &TERMS)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotFound);
}

#[test]
fn attest_rejects_double_attestation() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    let err = f
        .escrow
        .try_attest_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotPending);
}

// ----- release --------------------------------------------------------------

#[test]
fn release_is_blocked_while_window_is_open() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    f.advance(WINDOW - 10);

    let err = f
        .escrow
        .try_release_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::DisputeWindowOpen);
}

#[test]
fn release_pays_the_school_not_the_student() {
    let f = setup();
    let grant_id = f.create();
    f.attest_and_wait(grant_id, 0);

    f.escrow.release_term(&grant_id, &0);

    assert_eq!(f.token.balance(&f.school_payout), TERM_AMOUNT);
    // The leakage this protocol exists to prevent.
    assert_eq!(f.token.balance(&f.beneficiary), 0);
    assert_eq!(
        f.escrow.locked_amount(&grant_id),
        TERM_AMOUNT * i128::from(TERMS - 1)
    );
    assert_eq!(f.escrow.get_grant(&grant_id).next_term, 1);
    assert_eq!(f.escrow.get_term(&grant_id, &0).status, TermStatus::Released);
}

#[test]
fn release_requires_attestation_first() {
    let f = setup();
    let grant_id = f.create();
    let err = f
        .escrow
        .try_release_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotAttested);
}

#[test]
fn release_cannot_run_twice() {
    let f = setup();
    let grant_id = f.create();
    f.attest_and_wait(grant_id, 0);
    f.escrow.release_term(&grant_id, &0);

    let err = f
        .escrow
        .try_release_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotAttested);
}

#[test]
fn full_lifecycle_completes_the_grant() {
    let f = setup();
    let grant_id = f.create();

    for term in 0..TERMS {
        f.attest_and_wait(grant_id, term);
        f.escrow.release_term(&grant_id, &term);
    }

    let grant = f.escrow.get_grant(&grant_id);
    assert_eq!(grant.status, GrantStatus::Completed);
    assert_eq!(grant.next_term, TERMS);
    assert_eq!(f.token.balance(&f.school_payout), TERM_AMOUNT * i128::from(TERMS));
    assert_eq!(f.token.balance(&f.escrow.address), 0);
    assert_eq!(f.escrow.locked_amount(&grant_id), 0);
}

#[test]
fn completed_grant_rejects_further_attestation() {
    let f = setup();
    let grant_id = f.create();
    for term in 0..TERMS {
        f.attest_and_wait(grant_id, term);
        f.escrow.release_term(&grant_id, &term);
    }
    let err = f
        .escrow
        .try_attest_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::GrantNotActive);
}

#[test]
#[should_panic]
fn release_traps_for_a_suspended_institution() {
    let f = setup();
    let grant_id = f.create();
    f.attest_and_wait(grant_id, 0);

    // Fraud discovered between attestation and release.
    f.registry.suspend(&f.institution);

    // payout_of traps for a suspended institution, so the transfer cannot happen.
    f.escrow.release_term(&grant_id, &0);
}

// ----- disputes -------------------------------------------------------------

#[test]
fn dispute_freezes_the_term() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);

    f.escrow.dispute_term(&grant_id, &0);

    assert_eq!(f.escrow.get_term(&grant_id, &0).status, TermStatus::Disputed);

    // Even after the window elapses, a disputed term cannot be released.
    f.advance(WINDOW + 1);
    let err = f
        .escrow
        .try_release_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotAttested);
}

#[test]
fn dispute_is_rejected_after_window_closes() {
    let f = setup();
    let grant_id = f.create();
    f.attest_and_wait(grant_id, 0);

    let err = f
        .escrow
        .try_dispute_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::DisputeWindowClosed);
}

#[test]
fn dispute_requires_an_attested_term() {
    let f = setup();
    let grant_id = f.create();
    let err = f
        .escrow
        .try_dispute_term(&grant_id, &0)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotAttested);
}

#[test]
fn resolving_for_the_school_pays_out_and_advances() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    f.escrow.dispute_term(&grant_id, &0);

    f.escrow.resolve_dispute(&grant_id, &0, &true);

    assert_eq!(f.token.balance(&f.school_payout), TERM_AMOUNT);
    assert_eq!(f.token.balance(&f.sponsor), SPONSOR_FUNDING - TERM_AMOUNT * i128::from(TERMS));
    assert_eq!(f.escrow.get_term(&grant_id, &0).status, TermStatus::Released);
    assert_eq!(f.escrow.get_grant(&grant_id).next_term, 1);
}

#[test]
fn resolving_for_the_sponsor_refunds_and_advances() {
    let f = setup();
    let grant_id = f.create();
    let after_funding = f.token.balance(&f.sponsor);
    f.escrow.attest_term(&grant_id, &0);
    f.escrow.dispute_term(&grant_id, &0);

    f.escrow.resolve_dispute(&grant_id, &0, &false);

    assert_eq!(f.token.balance(&f.school_payout), 0);
    assert_eq!(f.token.balance(&f.sponsor), after_funding + TERM_AMOUNT);
    assert_eq!(f.escrow.get_term(&grant_id, &0).status, TermStatus::Refunded);
    // A refunded term still consumes its slot; the grant moves on.
    assert_eq!(f.escrow.get_grant(&grant_id).next_term, 1);
}

#[test]
fn resolve_requires_a_disputed_term() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    let err = f
        .escrow
        .try_resolve_dispute(&grant_id, &0, &true)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::TermNotDisputed);
}

#[test]
fn grant_can_finish_through_a_refunded_term() {
    let f = setup();
    let grant_id = f.create();

    // Term 0 disputed and refunded, terms 1..n released normally.
    f.escrow.attest_term(&grant_id, &0);
    f.escrow.dispute_term(&grant_id, &0);
    f.escrow.resolve_dispute(&grant_id, &0, &false);

    for term in 1..TERMS {
        f.attest_and_wait(grant_id, term);
        f.escrow.release_term(&grant_id, &term);
    }

    assert_eq!(f.escrow.get_grant(&grant_id).status, GrantStatus::Completed);
    assert_eq!(
        f.token.balance(&f.school_payout),
        TERM_AMOUNT * i128::from(TERMS - 1)
    );
    assert_eq!(f.token.balance(&f.escrow.address), 0);
}

// ----- cancellation ---------------------------------------------------------

#[test]
fn cancel_refunds_every_unsettled_term() {
    let f = setup();
    let grant_id = f.create();
    f.attest_and_wait(grant_id, 0);
    f.escrow.release_term(&grant_id, &0);

    f.escrow.cancel_grant(&grant_id);

    let grant = f.escrow.get_grant(&grant_id);
    assert_eq!(grant.status, GrantStatus::Cancelled);
    assert_eq!(f.token.balance(&f.escrow.address), 0);
    assert_eq!(f.token.balance(&f.school_payout), TERM_AMOUNT);
    assert_eq!(f.token.balance(&f.sponsor), SPONSOR_FUNDING - TERM_AMOUNT);
    assert_eq!(f.escrow.locked_amount(&grant_id), 0);
}

#[test]
fn cancel_is_refused_while_a_claim_is_open() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);

    let err = f
        .escrow
        .try_cancel_grant(&grant_id)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::SettlementInFlight);
}

#[test]
fn cancel_is_refused_while_a_dispute_is_open() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    f.escrow.dispute_term(&grant_id, &0);

    let err = f
        .escrow
        .try_cancel_grant(&grant_id)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::SettlementInFlight);
}

#[test]
fn cancel_cannot_run_twice() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.cancel_grant(&grant_id);
    let err = f
        .escrow
        .try_cancel_grant(&grant_id)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::GrantNotActive);
}

#[test]
fn cancelling_a_completed_grant_is_refused() {
    let f = setup();
    let grant_id = f.create();
    for term in 0..TERMS {
        f.attest_and_wait(grant_id, term);
        f.escrow.release_term(&grant_id, &term);
    }
    let err = f
        .escrow
        .try_cancel_grant(&grant_id)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::GrantNotActive);
}

// ----- configuration and lookups --------------------------------------------

#[test]
fn initialize_is_single_shot() {
    let f = setup();
    let err = f
        .escrow
        .try_initialize(&f.escrow_admin, &f.registry.address, &WINDOW)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::AlreadyInitialized);
}

#[test]
fn initialize_rejects_zero_dispute_window() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = TuitionEscrowClient::new(&env, &env.register(TuitionEscrow, ()));
    let err = escrow
        .try_initialize(&Address::generate(&env), &Address::generate(&env), &0u64)
        .err()
        .unwrap()
        .unwrap();
    assert_eq!(err, EscrowError::InvalidDisputeWindow);
}

#[test]
fn config_is_readable() {
    let f = setup();
    let config = f.escrow.get_config();
    assert_eq!(config.admin, f.escrow_admin);
    assert_eq!(config.registry, f.registry.address);
    assert_eq!(config.dispute_window, WINDOW);
}

#[test]
fn unknown_grant_is_reported() {
    let f = setup();
    let err = f.escrow.try_get_grant(&999).err().unwrap().unwrap();
    assert_eq!(err, EscrowError::GrantNotFound);
}

#[test]
fn unattested_terms_default_to_pending() {
    let f = setup();
    let grant_id = f.create();
    let term = f.escrow.get_term(&grant_id, &3);
    assert_eq!(term.status, TermStatus::Pending);
    assert_eq!(term.attested_at, 0);
    assert_eq!(term.release_after, 0);
}

// ----- authorisation --------------------------------------------------------

#[test]
#[should_panic]
fn attest_requires_the_institution_signature() {
    let f = setup();
    let grant_id = f.create();
    // Drop the blanket auth mock so require_auth is actually enforced.
    f.env.set_auths(&[]);
    f.escrow.attest_term(&grant_id, &0);
}

#[test]
#[should_panic]
fn cancel_requires_the_sponsor_signature() {
    let f = setup();
    let grant_id = f.create();
    f.env.set_auths(&[]);
    f.escrow.cancel_grant(&grant_id);
}

#[test]
#[should_panic]
fn resolve_dispute_requires_the_admin_signature() {
    let f = setup();
    let grant_id = f.create();
    f.escrow.attest_term(&grant_id, &0);
    f.escrow.dispute_term(&grant_id, &0);
    f.env.set_auths(&[]);
    f.escrow.resolve_dispute(&grant_id, &0, &true);
}
