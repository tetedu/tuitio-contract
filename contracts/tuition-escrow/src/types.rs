use soroban_sdk::{contracttype, Address};

/// Lifecycle of a whole grant.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GrantStatus {
    /// Terms remain to be settled.
    Active = 0,
    /// Every term was released or refunded.
    Completed = 1,
    /// The sponsor withdrew before all terms settled; remainder refunded.
    Cancelled = 2,
}

/// Lifecycle of a single term within a grant.
///
/// Terms settle strictly in order. A term moves
/// `Pending -> Attested -> Released`, or diverts through
/// `Disputed -> Released | Refunded` when the sponsor objects.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TermStatus {
    /// Awaiting the institution's attestation.
    Pending = 0,
    /// Institution attested; the sponsor's dispute window is running.
    Attested = 1,
    /// Sponsor objected inside the window; awaiting admin resolution.
    Disputed = 2,
    /// Funds transferred to the institution's payout address.
    Released = 3,
    /// Funds returned to the sponsor.
    Refunded = 4,
}

/// Protocol configuration, set once at initialization.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    /// Resolves disputes. Intentionally a distinct role from the registry admin.
    pub admin: Address,
    /// `institution-registry` contract consulted for payee legitimacy.
    pub registry: Address,
    /// Seconds a sponsor has to dispute after an attestation.
    pub dispute_window: u64,
}

/// A sponsor's commitment to fund one student at one institution.
///
/// The full amount (`term_amount * terms_total`) is transferred into the
/// contract at creation, so a grant is always fully collateralised. The
/// beneficiary never has a claim on the principal — only the institution's
/// registered payout address can receive it.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Grant {
    pub sponsor: Address,
    /// The student. Recorded for attribution and off-chain display only;
    /// this address can never receive escrowed funds.
    pub beneficiary: Address,
    /// Institution address as registered in `institution-registry`.
    pub institution: Address,
    /// SEP-41 token held in escrow, normally USDC.
    pub token: Address,
    /// Amount released per term, in the token's smallest unit.
    pub term_amount: i128,
    pub terms_total: u32,
    /// Index of the next term to settle. Equals `terms_total` when done.
    pub next_term: u32,
    pub status: GrantStatus,
    pub created_at: u64,
}

/// Per-term settlement state.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Term {
    pub status: TermStatus,
    /// Ledger timestamp of the institution's attestation, 0 if never attested.
    pub attested_at: u64,
    /// Earliest timestamp at which `release_term` may run, 0 if not attested.
    pub release_after: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    /// Next grant id to allocate.
    NextGrantId,
    /// Grant record by id.
    Grant(u64),
    /// Term record by (grant id, term index).
    Term(u64, u32),
}
