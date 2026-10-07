//! Typed contract events.
//!
//! Each struct below is one event. The indexer in `tuitio-backend` decodes
//! these by topic, so the field order and topic names here are a public
//! interface — changing them is a breaking change for downstream consumers.

use soroban_sdk::{contractevent, Address, String};

/// An institution submitted a registration. Status is `Pending` at this point.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstitutionRegistered {
    #[topic]
    pub institution: Address,
    pub payout: Address,
    pub name: String,
    pub country: String,
    pub registered_at: u64,
}

/// The admin promoted an institution to `Verified`. It can now receive tuition.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstitutionVerified {
    #[topic]
    pub institution: Address,
    pub verified_at: u64,
}

/// The admin suspended an institution. Existing grants are unaffected.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstitutionSuspended {
    #[topic]
    pub institution: Address,
    pub suspended_at: u64,
}

/// An institution rotated the address that receives tuition transfers.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutUpdated {
    #[topic]
    pub institution: Address,
    pub old_payout: Address,
    pub new_payout: Address,
}

/// Registry administration was transferred.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChanged {
    pub old_admin: Address,
    pub new_admin: Address,
}

/// The registry was configured. Gives indexers and explorers a genesis
/// record rather than having to infer deployment from configuration.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryInitialized {
    #[topic]
    pub admin: Address,
    pub initialized_at: u64,
}
