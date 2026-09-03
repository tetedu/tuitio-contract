//! Client-side view of the `institution-registry` contract.
//!
//! Declared locally rather than importing the registry crate so the escrow
//! can be deployed against any registry implementing this interface.

use soroban_sdk::{contractclient, Address, Env};

#[contractclient(name = "RegistryClient")]
pub trait RegistryInterface {
    /// True only when the institution exists and is `Verified`.
    fn is_verified(env: Env, institution: Address) -> bool;

    /// Payout address of a `Verified` institution. Traps if the institution
    /// is unknown, `Pending`, or `Suspended`.
    fn payout_of(env: Env, institution: Address) -> Address;
}
