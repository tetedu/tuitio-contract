#![no_std]
//! # Institution Registry
//!
//! Answers exactly one question for the rest of the protocol: *is this address
//! a legitimate tuition payee?*
//!
//! Registration is permissionless — any address may submit itself as `Pending`.
//! Only the registry admin can promote a record to `Verified`, and only
//! `Verified` institutions pass [`InstitutionRegistry::is_verified`], which the
//! tuition escrow calls before it will accept a grant.
//!
//! Suspension is reversible and does not claw back funds already escrowed; it
//! only prevents new grants from naming the institution.

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::RegistryError;
pub use types::{DataKey, Institution, InstitutionStatus, COUNTRY_LEN, MAX_NAME_LEN, MAX_PAGE};

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

/// Ledgers produced in roughly one day at ~5s close time.
const DAY_IN_LEDGERS: u32 = 17_280;
/// Extend the instance TTL when it drops below 30 days.
const INSTANCE_TTL_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
/// Extend the instance TTL out to 60 days.
const INSTANCE_TTL_EXTEND: u32 = DAY_IN_LEDGERS * 60;
/// Institution records are long-lived; keep them further out than the instance.
const ENTRY_TTL_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const ENTRY_TTL_EXTEND: u32 = DAY_IN_LEDGERS * 90;

#[contract]
pub struct InstitutionRegistry;

#[contractimpl]
impl InstitutionRegistry {
    /// Sets the registry administrator. Callable once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RegistryError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Count, &0u32);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
        Ok(())
    }

    /// Submits a self-registration. The record starts in [`InstitutionStatus::Pending`]
    /// and cannot receive tuition until the admin verifies it.
    ///
    /// Authorises: `caller`.
    pub fn register(
        env: Env,
        caller: Address,
        payout: Address,
        name: String,
        country: String,
    ) -> Result<(), RegistryError> {
        caller.require_auth();
        Self::require_init(&env)?;

        if name.is_empty() || name.len() > MAX_NAME_LEN || country.len() != COUNTRY_LEN {
            return Err(RegistryError::InvalidMetadata);
        }
        let key = DataKey::Institution(caller.clone());
        if env.storage().persistent().has(&key) {
            return Err(RegistryError::InstitutionExists);
        }

        let record = Institution {
            admin: caller.clone(),
            payout: payout.clone(),
            name: name.clone(),
            country: country.clone(),
            status: InstitutionStatus::Pending,
            registered_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);

        // Record the registration position so `list` can page through the
        // registry. Positions are append-only and never reused, so a page
        // taken now stays stable as more institutions register.
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        let position = DataKey::InstitutionAt(count);
        env.storage().persistent().set(&position, &caller);
        env.storage()
            .persistent()
            .extend_ttl(&position, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);

        env.storage().instance().set(&DataKey::Count, &(count + 1));
        Self::bump_instance(&env);

        events::InstitutionRegistered {
            institution: caller,
            payout,
            name,
            country,
            registered_at: record.registered_at,
        }
        .publish(&env);
        Ok(())
    }

    /// Promotes a `Pending` or `Suspended` institution to `Verified`.
    ///
    /// Authorises: registry admin.
    pub fn verify(env: Env, institution: Address) -> Result<(), RegistryError> {
        Self::require_admin(&env)?;
        let mut record = Self::load(&env, &institution)?;
        if record.status == InstitutionStatus::Verified {
            return Err(RegistryError::InvalidStatus);
        }
        record.status = InstitutionStatus::Verified;
        Self::store(&env, &institution, &record);
        events::InstitutionVerified {
            institution,
            verified_at: env.ledger().timestamp(),
        }
        .publish(&env);
        Ok(())
    }

    /// Moves a `Verified` institution to `Suspended`, blocking new grants.
    /// Does not affect grants already escrowed against it.
    ///
    /// Authorises: registry admin.
    pub fn suspend(env: Env, institution: Address) -> Result<(), RegistryError> {
        Self::require_admin(&env)?;
        let mut record = Self::load(&env, &institution)?;
        if record.status != InstitutionStatus::Verified {
            return Err(RegistryError::InvalidStatus);
        }
        record.status = InstitutionStatus::Suspended;
        Self::store(&env, &institution, &record);
        events::InstitutionSuspended {
            institution,
            suspended_at: env.ledger().timestamp(),
        }
        .publish(&env);
        Ok(())
    }

    /// Rotates the address that receives tuition transfers.
    ///
    /// Authorises: the institution's own `admin` address.
    pub fn update_payout(
        env: Env,
        institution: Address,
        new_payout: Address,
    ) -> Result<(), RegistryError> {
        let mut record = Self::load(&env, &institution)?;
        record.admin.require_auth();
        let old = record.payout.clone();
        record.payout = new_payout.clone();
        Self::store(&env, &institution, &record);
        events::PayoutUpdated {
            institution,
            old_payout: old,
            new_payout,
        }
        .publish(&env);
        Ok(())
    }

    /// Transfers registry administration.
    ///
    /// Authorises: current registry admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), RegistryError> {
        let old = Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Self::bump_instance(&env);
        events::AdminChanged {
            old_admin: old,
            new_admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Returns the full institution record.
    pub fn get_institution(env: Env, institution: Address) -> Result<Institution, RegistryError> {
        Self::load(&env, &institution)
    }

    /// Returns the payout address, but only for a `Verified` institution.
    /// The escrow uses this so a suspended school cannot be paid.
    pub fn payout_of(env: Env, institution: Address) -> Result<Address, RegistryError> {
        let record = Self::load(&env, &institution)?;
        if record.status != InstitutionStatus::Verified {
            return Err(RegistryError::InvalidStatus);
        }
        Ok(record.payout)
    }

    /// True only when the institution exists and is `Verified`.
    /// Returns `false` rather than erroring for unknown addresses so callers
    /// can use it as a plain predicate.
    pub fn is_verified(env: Env, institution: Address) -> bool {
        match Self::load(&env, &institution) {
            Ok(record) => record.status == InstitutionStatus::Verified,
            Err(_) => false,
        }
    }

    /// Returns institution addresses in registration order.
    ///
    /// `start` is a registration position, not an id: position 0 is the first
    /// institution that ever registered. Pages are stable because positions
    /// are append-only. A `start` past the end returns an empty vector rather
    /// than an error, so a caller can page until the result is short.
    ///
    /// `limit` is clamped to [`MAX_PAGE`] so one call cannot exceed the
    /// ledger's read budget.
    pub fn list(env: Env, start: u32, limit: u32) -> Vec<Address> {
        let total: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        let mut page = Vec::new(&env);
        if start >= total || limit == 0 {
            return page;
        }
        let capped = if limit > MAX_PAGE { MAX_PAGE } else { limit };
        let end = core::cmp::min(start.saturating_add(capped), total);

        for position in start..end {
            let key = DataKey::InstitutionAt(position);
            if let Some(address) = env.storage().persistent().get::<_, Address>(&key) {
                env.storage()
                    .persistent()
                    .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
                page.push_back(address);
            }
        }
        page
    }

    /// Number of institutions ever registered, verified or not.
    pub fn count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Count).unwrap_or(0)
    }

    /// Current registry administrator.
    pub fn admin(env: Env) -> Result<Address, RegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)
    }

    // ----- internal helpers -------------------------------------------------

    fn require_init(env: &Env) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            Ok(())
        } else {
            Err(RegistryError::NotInitialized)
        }
    }

    fn require_admin(env: &Env) -> Result<Address, RegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();
        Ok(admin)
    }

    fn load(env: &Env, institution: &Address) -> Result<Institution, RegistryError> {
        let key = DataKey::Institution(institution.clone());
        let record: Institution = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::InstitutionNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
        Ok(record)
    }

    fn store(env: &Env, institution: &Address, record: &Institution) {
        let key = DataKey::Institution(institution.clone());
        env.storage().persistent().set(&key, record);
        env.storage()
            .persistent()
            .extend_ttl(&key, ENTRY_TTL_THRESHOLD, ENTRY_TTL_EXTEND);
        Self::bump_instance(env);
    }

    fn bump_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
    }
}
