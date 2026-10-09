use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    /// `initialize` has already been called on this contract instance.
    AlreadyInitialized = 1,
    /// A public function was called before `initialize`.
    NotInitialized = 2,
    /// No grant exists under the supplied id.
    GrantNotFound = 3,
    /// The term index is outside the grant's term count.
    TermNotFound = 4,
    /// The named institution is not `Verified` in the registry.
    InstitutionNotVerified = 5,
    /// `term_amount` was zero or negative.
    InvalidAmount = 6,
    /// `terms_total` was zero.
    InvalidTermCount = 7,
    /// The grant is cancelled or already completed.
    GrantNotActive = 8,
    /// The term is not awaiting attestation.
    TermNotPending = 9,
    /// The term has not been attested by the institution.
    TermNotAttested = 10,
    /// The term is not under dispute.
    TermNotDisputed = 11,
    /// Terms must be settled in order; this is not the grant's next term.
    OutOfOrderTerm = 12,
    /// The sponsor's dispute window has not yet elapsed.
    DisputeWindowOpen = 13,
    /// The sponsor's dispute window has already closed.
    DisputeWindowClosed = 14,
    /// `term_amount * terms_total` overflowed i128.
    Overflow = 15,
    /// A term is attested or disputed, so the grant cannot be cancelled yet.
    SettlementInFlight = 16,
    /// The configured dispute window was zero.
    InvalidDisputeWindow = 17,
    /// More grants were passed to `sweep` than one transaction can settle.
    BatchTooLarge = 18,
}
