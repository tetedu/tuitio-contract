#![cfg(test)]
// Constants group digits to mirror USDC's 7-decimal precision (50_0000000 = 50 USDC).
#![allow(clippy::inconsistent_digit_grouping)]
//! Property tests over the escrow's state machine.
//!
//! The hand-written tests cover paths chosen by whoever wrote them, which is
//! exactly the weakness: a state machine with five term states, ordered
//! settlement and a time-dependent window has combinations nobody thinks to
//! try. These tests drive random but legal action sequences and assert the
//! invariants that must hold no matter what order things happen in.
//!
//! The invariant that matters most is conservation: every token that enters
//! the escrow must end up with the institution, back with the sponsor, or
//! still held by the contract — never created, never stranded.

use super::*;
use institution_registry::{InstitutionRegistry, InstitutionRegistryClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, String,
};

const TERM_AMOUNT: i128 = 50_0000000;
const WINDOW: u64 = 7 * 24 * 60 * 60;
const FUNDING: i128 = 1_000_000_0000000;

/// One legal move against a grant.
#[derive(Clone, Debug)]
enum Action {
    Attest,
    Release,
    Dispute,
    ResolveRelease,
    ResolveRefund,
    Cancel,
    /// Push the ledger clock forward far enough to close a dispute window.
    PassWindow,
    /// Push it forward, but not far enough.
    Tick,
}

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Attest),
        3 => Just(Action::Release),
        2 => Just(Action::Dispute),
        1 => Just(Action::ResolveRelease),
        1 => Just(Action::ResolveRefund),
        1 => Just(Action::Cancel),
        3 => Just(Action::PassWindow),
        2 => Just(Action::Tick),
    ]
}

struct World {
    env: Env,
    escrow: TuitionEscrowClient<'static>,
    token: token::TokenClient<'static>,
    sponsor: Address,
    school_payout: Address,
    escrow_address: Address,
    grant_id: u64,
    terms_total: u32,
}

fn world(terms_total: u32) -> World {
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
    token::StellarAssetClient::new(&env, &token_id).mint(&sponsor, &FUNDING);

    registry.register(
        &institution,
        &school_payout,
        &String::from_str(&env, "Property School"),
        &String::from_str(&env, "RW"),
    );
    registry.verify(&institution);

    let grant_id = escrow.create_grant(
        &sponsor,
        &beneficiary,
        &institution,
        &token_id,
        &TERM_AMOUNT,
        &terms_total,
    );
    let token = token::TokenClient::new(&env, &token_id);

    World {
        env,
        escrow_address: escrow.address.clone(),
        escrow,
        token,
        sponsor,
        school_payout,
        grant_id,
        terms_total,
    }
}

impl World {
    /// Every token is either with the sponsor, with the school, or still in
    /// escrow. Nothing is minted and nothing disappears.
    fn assert_conservation(&self) {
        let total = self.token.balance(&self.sponsor)
            + self.token.balance(&self.school_payout)
            + self.token.balance(&self.escrow_address);
        assert_eq!(
            total,
            FUNDING,
            "tokens were created or destroyed: sponsor {} + school {} + escrow {} != {}",
            self.token.balance(&self.sponsor),
            self.token.balance(&self.school_payout),
            self.token.balance(&self.escrow_address),
            FUNDING
        );
    }

    /// What the contract still holds must equal what it still owes on this
    /// grant: the unsettled terms of an active grant, and nothing once the
    /// grant is finished.
    fn assert_escrow_matches_liability(&self) {
        let grant = self.escrow.get_grant(&self.grant_id);
        let expected = match grant.status {
            GrantStatus::Active => TERM_AMOUNT * i128::from(grant.terms_total - grant.next_term),
            _ => 0,
        };
        assert_eq!(
            self.token.balance(&self.escrow_address),
            expected,
            "escrow holds {} but owes {} (status {:?}, next_term {}/{})",
            self.token.balance(&self.escrow_address),
            expected,
            grant.status,
            grant.next_term,
            grant.terms_total
        );
        assert_eq!(
            self.escrow.locked_amount(&self.grant_id),
            expected,
            "locked_amount disagrees with the contract's own balance"
        );
    }

    /// Settlement is ordered: everything before `next_term` is final, and
    /// everything after it is untouched.
    fn assert_terms_consistent(&self) {
        let grant = self.escrow.get_grant(&self.grant_id);
        for index in 0..self.terms_total {
            let term = self.escrow.get_term(&self.grant_id, &index);
            if index < grant.next_term {
                assert!(
                    matches!(term.status, TermStatus::Released | TermStatus::Refunded),
                    "settled term {} is {:?}, expected Released or Refunded",
                    index,
                    term.status
                );
            } else if index > grant.next_term {
                assert_eq!(
                    term.status,
                    TermStatus::Pending,
                    "term {} was touched out of order",
                    index
                );
            }
        }
    }

    fn apply(&self, action: &Action) {
        let grant = self.escrow.get_grant(&self.grant_id);
        let index = grant.next_term.min(self.terms_total.saturating_sub(1));
        match action {
            // Every call goes through try_ because most actions are illegal in
            // most states; the point is that an illegal call is rejected
            // cleanly and changes nothing, not that the test only makes legal
            // moves.
            Action::Attest => {
                let _ = self.escrow.try_attest_term(&self.grant_id, &index);
            }
            Action::Release => {
                let _ = self.escrow.try_release_term(&self.grant_id, &index);
            }
            Action::Dispute => {
                let _ = self.escrow.try_dispute_term(&self.grant_id, &index);
            }
            Action::ResolveRelease => {
                let _ = self
                    .escrow
                    .try_resolve_dispute(&self.grant_id, &index, &true);
            }
            Action::ResolveRefund => {
                let _ = self
                    .escrow
                    .try_resolve_dispute(&self.grant_id, &index, &false);
            }
            Action::Cancel => {
                let _ = self.escrow.try_cancel_grant(&self.grant_id);
            }
            Action::PassWindow => {
                let now = self.env.ledger().timestamp();
                self.env
                    .ledger()
                    .with_mut(|l| l.timestamp = now + WINDOW + 1);
            }
            Action::Tick => {
                let now = self.env.ledger().timestamp();
                self.env.ledger().with_mut(|l| l.timestamp = now + 60);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Drive a random legal-or-rejected action sequence and assert the
    /// invariants after every single step, so a failure points at the exact
    /// move that broke them.
    #[test]
    fn escrow_invariants_hold_under_any_action_sequence(
        terms_total in 1u32..5,
        actions in proptest::collection::vec(action_strategy(), 1..24),
    ) {
        let w = world(terms_total);
        w.assert_conservation();
        w.assert_escrow_matches_liability();

        for action in &actions {
            w.apply(action);
            w.assert_conservation();
            w.assert_escrow_matches_liability();
            w.assert_terms_consistent();
        }
    }

    /// `next_term` must never move backwards, whatever happens.
    #[test]
    fn next_term_never_regresses(
        terms_total in 1u32..5,
        actions in proptest::collection::vec(action_strategy(), 1..24),
    ) {
        let w = world(terms_total);
        let mut highest = 0u32;
        for action in &actions {
            w.apply(action);
            let next = w.escrow.get_grant(&w.grant_id).next_term;
            prop_assert!(
                next >= highest,
                "next_term went backwards: {} after {}",
                next,
                highest
            );
            prop_assert!(next <= terms_total, "next_term {} exceeds total", next);
            highest = next;
        }
    }

    /// A grant that reports Completed must have paid out or refunded every
    /// term and be holding nothing.
    #[test]
    fn a_completed_grant_holds_nothing(
        terms_total in 1u32..4,
        actions in proptest::collection::vec(action_strategy(), 1..32),
    ) {
        let w = world(terms_total);
        for action in &actions {
            w.apply(action);
        }
        let grant = w.escrow.get_grant(&w.grant_id);
        if grant.status == GrantStatus::Completed {
            prop_assert_eq!(grant.next_term, terms_total);
            prop_assert_eq!(w.token.balance(&w.escrow_address), 0);
            prop_assert_eq!(w.escrow.locked_amount(&w.grant_id), 0);
        }
    }
}
