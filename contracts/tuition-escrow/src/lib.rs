#![no_std]
//! # Tuition Escrow
//!
//! Holds sponsor funds and releases them, one academic term at a time, only to
//! an institution that `institution-registry` marks `Verified`.
//!
//! ## Why the beneficiary never holds the principal
//!
//! The problem this contract exists to solve is leakage: education money sent
//! to a household arrives as general cash and gets spent on more urgent needs.
//! So the student is recorded as `beneficiary` for attribution, but the only
//! address that can ever receive escrowed funds is the institution's registered
//! payout address.
//!
//! ## The attestation conflict, stated plainly
//!
//! The institution both attests that a term happened and is the payee. That is
//! a genuine conflict of interest and this contract does not pretend to remove
//! it. It constrains it three ways:
//!
//! 1. Attesting does not move money. It starts a `dispute_window` timer.
//! 2. The sponsor can dispute inside that window, freezing the term.
//! 3. Registry verification gates who may be named at all, and suspension
//!    blocks release because `payout_of` traps for a suspended institution.
//!
//! A suspended institution's attested term therefore cannot be released. The
//! sponsor disputes it and the admin refunds — funds are never stranded.

mod errors;
mod events;
mod registry;
mod types;

#[cfg(test)]
mod test;

pub use errors::EscrowError;
pub use registry::{RegistryClient, RegistryInterface};
pub use types::{Config, DataKey, Grant, GrantStatus, Term, TermStatus};

use soroban_sdk::{contract, contractimpl, token, Address, Env};

const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const INSTANCE_TTL_EXTEND: u32 = DAY_IN_LEDGERS * 60;
/// Grants outlive the instance bump cadence; a multi-year degree is in scope.
const ENTRY_TTL_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const ENTRY_TTL_EXTEND: u32 = DAY_IN_LEDGERS * 90;

#[contract]
pub struct TuitionEscrow;

#[contractimpl]
impl TuitionEscrow {
    /// Configures the escrow. Callable once.
    ///
    /// `admin` resolves disputes and is deliberately a separate role from the
    /// registry admin. `dispute_window` is in seconds and must be non-zero —
    /// a zero window would let an institution attest and release atomically,
    /// removing the sponsor's only check.
    pub fn initialize(
        env: Env,
        admin: Address,
        registry: Address,
        dispute_window: u64,
    ) -> Result<(), EscrowError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(EscrowError::AlreadyInitialized);
        }
        if dispute_window == 0 {
            return Err(EscrowError::InvalidDisputeWindow);
        }
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admin: admin.clone(),
                registry: registry.clone(),
                dispute_window,
            },
        );
        env.storage().instance().set(&DataKey::NextGrantId, &0u64);
        Self::bump_instance(&env);

        events::EscrowInitialized {
            admin,
            registry,
            dispute_window,
            initialized_at: env.ledger().timestamp(),
        }
        .publish(&env);
        Ok(())
    }

    /// Creates a grant and pulls the full commitment into escrow.
    ///
    /// Transfers `term_amount * terms_total` from `sponsor` to this contract,
    /// so a grant is fully collateralised from the moment it exists. A sponsor
    /// cannot promise money it does not have.
    ///
    /// Authorises: `sponsor`.
    pub fn create_grant(
        env: Env,
        sponsor: Address,
        beneficiary: Address,
        institution: Address,
        token_id: Address,
        term_amount: i128,
        terms_total: u32,
    ) -> Result<u64, EscrowError> {
        sponsor.require_auth();
        let config = Self::config(&env)?;

        if term_amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }
        if terms_total == 0 {
            return Err(EscrowError::InvalidTermCount);
        }

        let registry_client = RegistryClient::new(&env, &config.registry);
        if !registry_client.is_verified(&institution) {
            return Err(EscrowError::InstitutionNotVerified);
        }

        let total = term_amount
            .checked_mul(i128::from(terms_total))
            .ok_or(EscrowError::Overflow)?;

        token::TokenClient::new(&env, &token_id).transfer(
            &sponsor,
            env.current_contract_address(),
            &total,
        );

        let grant_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextGrantId)
            .unwrap_or(0);
        let grant = Grant {
            sponsor: sponsor.clone(),
            beneficiary: beneficiary.clone(),
            institution: institution.clone(),
            token: token_id.clone(),
            term_amount,
            terms_total,
            next_term: 0,
            status: GrantStatus::Active,
            created_at: env.ledger().timestamp(),
        };
        Self::put_grant(&env, grant_id, &grant);
        env.storage()
            .instance()
            .set(&DataKey::NextGrantId, &(grant_id + 1));
        Self::bump_instance(&env);

        events::GrantCreated {
            grant_id,
            sponsor,
            institution,
            beneficiary,
            token: token_id,
            term_amount,
            terms_total,
            total_funded: total,
        }
        .publish(&env);

        Ok(grant_id)
    }

    /// Records that the beneficiary completed a term, starting the dispute
    /// window. Moves no funds.
    ///
    /// Terms must be attested in order, so an institution cannot claim term 5
    /// while term 2 is unresolved.
    ///
    /// Authorises: the grant's `institution` address.
    pub fn attest_term(env: Env, grant_id: u64, term_index: u32) -> Result<(), EscrowError> {
        let config = Self::config(&env)?;
        let grant = Self::get_grant_or_err(&env, grant_id)?;
        grant.institution.require_auth();

        Self::require_active(&grant)?;
        Self::require_in_range(&grant, term_index)?;
        if term_index != grant.next_term {
            return Err(EscrowError::OutOfOrderTerm);
        }

        let mut term = Self::load_term(&env, grant_id, term_index);
        if term.status != TermStatus::Pending {
            return Err(EscrowError::TermNotPending);
        }

        let now = env.ledger().timestamp();
        term.status = TermStatus::Attested;
        term.attested_at = now;
        term.release_after = now + config.dispute_window;
        Self::put_term(&env, grant_id, term_index, &term);

        events::TermAttested {
            grant_id,
            term_index,
            institution: grant.institution,
            attested_at: now,
            release_after: term.release_after,
        }
        .publish(&env);
        Ok(())
    }

    /// Pays one attested term to the institution's payout address.
    ///
    /// Permissionless on purpose: once the dispute window has elapsed without
    /// objection, the transfer is owed, and no party should be able to stall it
    /// by withholding a signature.
    ///
    /// Traps if the institution has since been suspended — see the module docs
    /// for how that unwinds.
    pub fn release_term(env: Env, grant_id: u64, term_index: u32) -> Result<(), EscrowError> {
        let config = Self::config(&env)?;
        let mut grant = Self::get_grant_or_err(&env, grant_id)?;
        Self::require_active(&grant)?;
        Self::require_in_range(&grant, term_index)?;

        let mut term = Self::load_term(&env, grant_id, term_index);
        if term.status != TermStatus::Attested {
            return Err(EscrowError::TermNotAttested);
        }
        if env.ledger().timestamp() < term.release_after {
            return Err(EscrowError::DisputeWindowOpen);
        }

        let payout = RegistryClient::new(&env, &config.registry).payout_of(&grant.institution);
        Self::pay(&env, &grant.token, &payout, grant.term_amount);

        term.status = TermStatus::Released;
        Self::put_term(&env, grant_id, term_index, &term);
        Self::advance(&env, grant_id, &mut grant);

        events::TermReleased {
            grant_id,
            term_index,
            payout,
            amount: grant.term_amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Freezes an attested term before its window closes.
    ///
    /// Authorises: the grant's `sponsor`.
    pub fn dispute_term(env: Env, grant_id: u64, term_index: u32) -> Result<(), EscrowError> {
        Self::config(&env)?;
        let grant = Self::get_grant_or_err(&env, grant_id)?;
        grant.sponsor.require_auth();

        Self::require_active(&grant)?;
        Self::require_in_range(&grant, term_index)?;

        let mut term = Self::load_term(&env, grant_id, term_index);
        if term.status != TermStatus::Attested {
            return Err(EscrowError::TermNotAttested);
        }
        if env.ledger().timestamp() >= term.release_after {
            return Err(EscrowError::DisputeWindowClosed);
        }

        term.status = TermStatus::Disputed;
        Self::put_term(&env, grant_id, term_index, &term);

        events::TermDisputed {
            grant_id,
            term_index,
            sponsor: grant.sponsor,
            disputed_at: env.ledger().timestamp(),
        }
        .publish(&env);
        Ok(())
    }

    /// Settles a disputed term, either paying the institution or refunding the
    /// sponsor. Either way the term is final and the grant advances.
    ///
    /// Authorises: the escrow `admin`.
    pub fn resolve_dispute(
        env: Env,
        grant_id: u64,
        term_index: u32,
        release: bool,
    ) -> Result<(), EscrowError> {
        let config = Self::config(&env)?;
        config.admin.require_auth();

        let mut grant = Self::get_grant_or_err(&env, grant_id)?;
        Self::require_active(&grant)?;
        Self::require_in_range(&grant, term_index)?;

        let mut term = Self::load_term(&env, grant_id, term_index);
        if term.status != TermStatus::Disputed {
            return Err(EscrowError::TermNotDisputed);
        }

        if release {
            let payout = RegistryClient::new(&env, &config.registry).payout_of(&grant.institution);
            Self::pay(&env, &grant.token, &payout, grant.term_amount);
            term.status = TermStatus::Released;
        } else {
            Self::pay(&env, &grant.token, &grant.sponsor, grant.term_amount);
            term.status = TermStatus::Refunded;
            events::TermRefunded {
                grant_id,
                term_index,
                sponsor: grant.sponsor.clone(),
                amount: grant.term_amount,
            }
            .publish(&env);
        }
        Self::put_term(&env, grant_id, term_index, &term);
        Self::advance(&env, grant_id, &mut grant);

        events::DisputeResolved {
            grant_id,
            term_index,
            released: release,
            amount: grant.term_amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Withdraws every unsettled term and closes the grant.
    ///
    /// Refused while a term is attested or disputed: the institution has
    /// already done the work it is claiming for, so the sponsor cannot pull
    /// funds out from under an open claim.
    ///
    /// Authorises: the grant's `sponsor`.
    pub fn cancel_grant(env: Env, grant_id: u64) -> Result<(), EscrowError> {
        Self::config(&env)?;
        let mut grant = Self::get_grant_or_err(&env, grant_id)?;
        grant.sponsor.require_auth();
        Self::require_active(&grant)?;

        if grant.next_term < grant.terms_total {
            let current = Self::load_term(&env, grant_id, grant.next_term);
            if current.status != TermStatus::Pending {
                return Err(EscrowError::SettlementInFlight);
            }
        }

        let remaining_terms = grant.terms_total - grant.next_term;
        let refund = grant
            .term_amount
            .checked_mul(i128::from(remaining_terms))
            .ok_or(EscrowError::Overflow)?;

        if refund > 0 {
            Self::pay(&env, &grant.token, &grant.sponsor, refund);
        }

        grant.status = GrantStatus::Cancelled;
        Self::put_grant(&env, grant_id, &grant);

        events::GrantCancelled {
            grant_id,
            sponsor: grant.sponsor,
            terms_refunded: remaining_terms,
            amount_refunded: refund,
        }
        .publish(&env);
        Ok(())
    }

    // ----- views ------------------------------------------------------------

    pub fn get_grant(env: Env, grant_id: u64) -> Result<Grant, EscrowError> {
        Self::get_grant_or_err(&env, grant_id)
    }

    pub fn get_term(env: Env, grant_id: u64, term_index: u32) -> Result<Term, EscrowError> {
        let grant = Self::get_grant_or_err(&env, grant_id)?;
        Self::require_in_range(&grant, term_index)?;
        Ok(Self::load_term(&env, grant_id, term_index))
    }

    /// Amount still held in escrow for a grant.
    pub fn locked_amount(env: Env, grant_id: u64) -> Result<i128, EscrowError> {
        let grant = Self::get_grant_or_err(&env, grant_id)?;
        if grant.status != GrantStatus::Active {
            return Ok(0);
        }
        grant
            .term_amount
            .checked_mul(i128::from(grant.terms_total - grant.next_term))
            .ok_or(EscrowError::Overflow)
    }

    pub fn get_config(env: Env) -> Result<Config, EscrowError> {
        Self::config(&env)
    }

    pub fn next_grant_id(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::NextGrantId)
            .unwrap_or(0)
    }

    // ----- internal ---------------------------------------------------------

    fn config(env: &Env) -> Result<Config, EscrowError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(EscrowError::NotInitialized)
    }

    fn require_active(grant: &Grant) -> Result<(), EscrowError> {
        if grant.status == GrantStatus::Active {
            Ok(())
        } else {
            Err(EscrowError::GrantNotActive)
        }
    }

    fn require_in_range(grant: &Grant, term_index: u32) -> Result<(), EscrowError> {
        if term_index < grant.terms_total {
            Ok(())
        } else {
            Err(EscrowError::TermNotFound)
        }
    }

    fn get_grant_or_err(env: &Env, grant_id: u64) -> Result<Grant, EscrowError> {
        let key = DataKey::Grant(grant_id);
        let grant: Grant = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(EscrowError::GrantNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
        Ok(grant)
    }

    fn put_grant(env: &Env, grant_id: u64, grant: &Grant) {
        let key = DataKey::Grant(grant_id);
        env.storage().persistent().set(&key, grant);
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
    }

    /// Terms default to `Pending` rather than being written at creation, so a
    /// 12-term grant costs one storage entry instead of thirteen.
    fn load_term(env: &Env, grant_id: u64, term_index: u32) -> Term {
        env.storage()
            .persistent()
            .get(&DataKey::Term(grant_id, term_index))
            .unwrap_or(Term {
                status: TermStatus::Pending,
                attested_at: 0,
                release_after: 0,
            })
    }

    fn put_term(env: &Env, grant_id: u64, term_index: u32, term: &Term) {
        let key = DataKey::Term(grant_id, term_index);
        env.storage().persistent().set(&key, term);
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
    }

    fn pay(env: &Env, token_id: &Address, to: &Address, amount: i128) {
        token::TokenClient::new(env, token_id).transfer(
            &env.current_contract_address(),
            to,
            &amount,
        );
    }

    /// Moves the grant to its next term, completing it when none remain.
    fn advance(env: &Env, grant_id: u64, grant: &mut Grant) {
        grant.next_term += 1;
        if grant.next_term == grant.terms_total {
            grant.status = GrantStatus::Completed;
            events::GrantCompleted {
                grant_id,
                terms_total: grant.terms_total,
            }
            .publish(env);
        }
        Self::put_grant(env, grant_id, grant);
    }

    fn bump_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
    }
}
