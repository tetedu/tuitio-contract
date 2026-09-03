//! Typed contract events consumed by the `tuitio-backend` indexer.
//!
//! Topic names and field order are a public interface. Changing them breaks
//! downstream consumers and requires an indexer migration.

use soroban_sdk::{contractevent, Address};

/// A sponsor created and fully funded a grant.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantCreated {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub institution: Address,
    pub beneficiary: Address,
    pub token: Address,
    pub term_amount: i128,
    pub terms_total: u32,
    pub total_funded: i128,
}

/// An institution attested that a term was completed. Starts the dispute window.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermAttested {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub term_index: u32,
    pub institution: Address,
    pub attested_at: u64,
    pub release_after: u64,
}

/// Term funds moved to the institution's payout address.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermReleased {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub term_index: u32,
    pub payout: Address,
    pub amount: i128,
}

/// Sponsor objected to an attestation inside the dispute window.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermDisputed {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub term_index: u32,
    pub sponsor: Address,
    pub disputed_at: u64,
}

/// Admin resolved a disputed term.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeResolved {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub term_index: u32,
    /// True when funds went to the institution, false when refunded to sponsor.
    pub released: bool,
    pub amount: i128,
}

/// Term funds returned to the sponsor.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermRefunded {
    #[topic]
    pub grant_id: u64,
    #[topic]
    pub term_index: u32,
    pub sponsor: Address,
    pub amount: i128,
}

/// Sponsor cancelled a grant; unsettled terms were refunded in one transfer.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantCancelled {
    #[topic]
    pub grant_id: u64,
    pub sponsor: Address,
    pub terms_refunded: u32,
    pub amount_refunded: i128,
}

/// Every term of a grant has settled.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantCompleted {
    #[topic]
    pub grant_id: u64,
    pub terms_total: u32,
}
