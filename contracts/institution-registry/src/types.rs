use soroban_sdk::{contracttype, Address, String};

/// Lifecycle of a registered institution.
///
/// `Pending` institutions cannot receive tuition disbursements. Only the
/// registry admin can move an institution to `Verified`, and only a
/// `Verified` institution passes `is_verified`.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InstitutionStatus {
    Pending = 0,
    Verified = 1,
    Suspended = 2,
}

/// A school, university, or training provider eligible to receive tuition.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Institution {
    /// Address controlling this record. Authorises payout changes.
    pub admin: Address,
    /// Address that actually receives tuition transfers.
    pub payout: Address,
    /// Legal or trading name, for off-chain display.
    pub name: String,
    /// ISO 3166-1 alpha-2 country code, for off-chain display and filtering.
    pub country: String,
    pub status: InstitutionStatus,
    /// Ledger timestamp at registration.
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Registry administrator address.
    Admin,
    /// Institution record, keyed by the institution's controlling address.
    Institution(Address),
    /// Monotonic count of registered institutions.
    Count,
}

/// Maximum accepted length for the `name` field, in bytes.
pub const MAX_NAME_LEN: u32 = 128;
/// Exact accepted length for the `country` field (ISO alpha-2), in bytes.
pub const COUNTRY_LEN: u32 = 2;
