use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// `initialize` has already been called on this contract instance.
    AlreadyInitialized = 1,
    /// A public function was called before `initialize`.
    NotInitialized = 2,
    /// Caller is not the registry admin.
    NotAdmin = 3,
    /// No institution is registered under the supplied address.
    InstitutionNotFound = 4,
    /// An institution is already registered under the supplied address.
    InstitutionExists = 5,
    /// The institution is not in a state that allows this transition.
    InvalidStatus = 6,
    /// A supplied string field was empty or exceeded its maximum length.
    InvalidMetadata = 7,
}
